mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Utc, Weekday, Datelike};
use common::{TestApp, AuthHeaders};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHasher, SaltString
    },
    Argon2,
};

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_setup(app: &TestApp, slug: &str) -> (String, String, AuthHeaders) {
    let tenant_id = Uuid::new_v4().to_string();
    let user_id = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES (?, ?, ?, ?)")
        .bind(&tenant_id)
        .bind("Time Corp")
        .bind(slug)
        .bind(Utc::now())
        .execute(&app.pool)
        .await
        .unwrap();

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default().hash_password(b"password", &salt).unwrap().to_string();

    sqlx::query("INSERT INTO users (id, tenant_id, username, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&user_id)
        .bind(&tenant_id)
        .bind("admin")
        .bind(password_hash)
        .bind("ADMIN")
        .bind(Utc::now())
        .execute(&app.pool)
        .await
        .unwrap();

    let auth = app.login(&tenant_id, "admin", "password").await;

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug": "time-ev",
                "title_en": "Time Test", "title_de": "Zeit Test", "desc_en": ".", "desc_de": ".",
                "location": "Loc", "payout": "0", "host_name": "H",
                "timezone": "UTC",
                "active_start": Utc::now().to_rfc3339(),
                "active_end": (Utc::now() + Duration::days(365)).to_rfc3339(),
                "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
                "config": {
                    "monday": [{"start":"00:00", "end":"23:59"}],
                    "tuesday": [{"start":"00:00", "end":"23:59"}],
                    "wednesday": [{"start":"00:00", "end":"23:59"}],
                    "thursday": [{"start":"00:00", "end":"23:59"}],
                    "friday": [{"start":"00:00", "end":"23:59"}],
                    "saturday": [{"start":"00:00", "end":"23:59"}],
                    "sunday": [{"start":"00:00", "end":"23:59"}]
                },
                "access_mode": "OPEN"
            }).to_string())).unwrap()
    ).await.unwrap();

    if !res.status().is_success() {
        let body = parse_body(res).await;
        panic!("Failed to create event: {:?}", body);
    }

    (tenant_id, "time-ev".to_string(), auth)
}

#[tokio::test]
async fn test_local_to_utc_conversion() {
    let app = TestApp::new().await;
    let (tid, slug, auth) = create_setup(&app, "utc-conv").await;

    let mut target_date = Utc::now();
    while target_date.weekday() != Weekday::Wed { target_date += Duration::days(1); }
    target_date += Duration::days(7);

    let date_str = target_date.format("%Y-%m-%d").to_string();
    let time_str = "12:00";

    let payload = json!({
        "date": date_str,
        "time": time_str,
        "name": "Time Traveler",
        "email": "tt@example.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_past_time_rejection() {
    let app = TestApp::new().await;
    let (tid, slug, auth) = create_setup(&app, "past-check").await;

    let past = Utc::now() - Duration::hours(2);
    let date_str = past.format("%Y-%m-%d").to_string();
    let time_str = past.format("%H:%M").to_string();

    let payload = json!({
        "date": date_str,
        "time": time_str,
        "name": "Late Person",
        "email": "late@example.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_duration_and_end_time_calculation() {
    let app = TestApp::new().await;
    let (tid, slug, auth) = create_setup(&app, "dur-calc").await;

    let mut target = Utc::now();
    target += Duration::days(2);
    let date_str = target.format("%Y-%m-%d").to_string();
    let time_str = "10:00";

    let payload = json!({
        "date": date_str,
        "time": time_str,
        "name": "Duration Check",
        "email": "dc@example.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;

    let start_ts = chrono::DateTime::parse_from_rfc3339(body["start_time"].as_str().unwrap()).unwrap();
    let end_ts = chrono::DateTime::parse_from_rfc3339(body["end_time"].as_str().unwrap()).unwrap();

    let diff = end_ts - start_ts;
    assert_eq!(diff.num_minutes(), 60);
}

#[tokio::test]
async fn test_slot_consistency_output() {
    let app = TestApp::new().await;
    let (tid, slug, auth) = create_setup(&app, "slot-out").await;

    let mut target = Utc::now();
    while target.weekday() != Weekday::Fri { target += Duration::days(1); }
    target += Duration::days(7);
    let date_str = target.format("%Y-%m-%d").to_string();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date_str))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    assert!(!slots.is_empty());
    // Check for ISO format existence
    assert!(slots.iter().any(|s| s.as_str().unwrap().contains("T09:00:00")));
}

#[tokio::test]
async fn test_date_boundary_booking() {
    let app = TestApp::new().await;
    let (tid, slug, auth) = create_setup(&app, "boundary").await;

    let mut target = Utc::now();
    target += Duration::days(5);
    let date_str = target.format("%Y-%m-%d").to_string();

    let time_str = "22:00";

    let payload = json!({
        "date": date_str,
        "time": time_str,
        "name": "Night Owl",
        "email": "owl@example.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}