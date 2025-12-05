mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Utc};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn setup_bitmask_test(app: &TestApp, slug: &str, config: Value) -> (String, String, String) {
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Bitmask Inc", "slug": format!("bm-{}", slug)}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();

    let auth = app.login(&tid, "admin", &sec).await;

    let ev_payload = json!({
        "slug": slug,
        "title_en": "Bitmask Test", "title_de": "Bitmask Test", "desc_en": ".", "desc_de": ".",
        "location": "HQ", "payout": "0", "host_name": "Host", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(60)).to_rfc3339(),
        "duration_min": 60, "interval_min": 30, "max_participants": 1, "image_url": ".",
        "config": config,
        "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    (tid, sec, slug.to_string())
}

fn get_target_date(offset_days: i64) -> String {
    let mut next = Utc::now();
    next += Duration::days(1);
    while next.format("%A").to_string() != "Monday" { next += Duration::days(1); }
    (next + Duration::days(offset_days)).format("%Y-%m-%d").to_string()
}

#[tokio::test]
async fn test_bitmask_case_1_empty_day() {
    let app = TestApp::new().await;
    let config = json!({ "monday": [{"start":"09:00","end":"12:00"}] });
    let (tid, _, slug) = setup_bitmask_test(&app, "case1", config).await;
    let date = get_target_date(0);

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date)).body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // Slots: 09:00, 09:30, 10:00, 10:30, 11:00 (UTC ISO strings)
    assert_eq!(slots.len(), 5);
    assert!(slots[0].as_str().unwrap().contains("T09:00:00"));
    assert!(slots[4].as_str().unwrap().contains("T11:00:00"));
}

#[tokio::test]
async fn test_bitmask_case_2_full_override_unavailable() {
    let app = TestApp::new().await;
    let config = json!({ "monday": [{"start":"09:00","end":"12:00"}] });
    let (tid, sec, slug) = setup_bitmask_test(&app, "case2", config).await;
    let date = get_target_date(0);
    let auth = app.login(&tid, "admin", &sec).await;

    let override_payload = json!({ "date": date, "is_unavailable": true, "config": null });
    app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug)).header(header::COOKIE, format!("access_token={}", auth.access_token)).header("X-CSRF-Token", &auth.csrf_token).header("Content-Type", "application/json").body(Body::from(override_payload.to_string())).unwrap()).await.unwrap();

    let res = app.router.clone().oneshot(Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date)).body(Body::empty()).unwrap()).await.unwrap();
    let body = parse_body(res).await;
    assert!(body["slots"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_bitmask_case_3_override_new_hours() {
    let app = TestApp::new().await;
    let config = json!({ "monday": [{"start":"09:00","end":"12:00"}] });
    let (tid, sec, slug) = setup_bitmask_test(&app, "case3", config).await;
    let date = get_target_date(0);
    let auth = app.login(&tid, "admin", &sec).await;

    let override_payload = json!({
        "date": date, "is_unavailable": false,
        "config": { "monday": [{"start": "13:00", "end": "14:30"}] }
    });
    app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug)).header(header::COOKIE, format!("access_token={}", auth.access_token)).header("X-CSRF-Token", &auth.csrf_token).header("Content-Type", "application/json").body(Body::from(override_payload.to_string())).unwrap()).await.unwrap();

    let res = app.router.clone().oneshot(Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date)).body(Body::empty()).unwrap()).await.unwrap();
    let slots = parse_body(res).await["slots"].as_array().unwrap().clone();

    assert_eq!(slots.len(), 2);
    // 13:00, 13:30 UTC
    assert!(slots[0].as_str().unwrap().contains("T13:00:00"));
    assert!(slots[1].as_str().unwrap().contains("T13:30:00"));
}

#[tokio::test]
async fn test_bitmask_case_4_booking_in_middle() {
    let app = TestApp::new().await;
    let config = json!({ "monday": [{"start":"09:00","end":"12:00"}] });
    let (tid, _, slug) = setup_bitmask_test(&app, "case4", config).await;
    let date = get_target_date(0);

    // Book 10:00-11:00
    let b_res = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug)).header("Content-Type", "application/json").body(Body::from(json!({"date": date, "time": "10:00", "name":"X", "email":"x@x.com"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(b_res.status(), StatusCode::OK);

    let res = app.router.clone().oneshot(Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date)).body(Body::empty()).unwrap()).await.unwrap();
    let slots = parse_body(res).await["slots"].as_array().unwrap().clone();

    // We iterate and check existence. ISO strings complicate direct contains.
    let has_0900 = slots.iter().any(|s| s.as_str().unwrap().contains("T09:00:00"));
    let has_1000 = slots.iter().any(|s| s.as_str().unwrap().contains("T10:00:00"));
    let has_1100 = slots.iter().any(|s| s.as_str().unwrap().contains("T11:00:00"));

    assert!(has_0900);
    assert!(!has_1000);
    assert!(has_1100);
}

#[tokio::test]
async fn test_bitmask_case_5_overlapping_start() {
    let app = TestApp::new().await;
    let config = json!({ "monday": [{"start":"09:00","end":"12:00"}] });
    let (tid, _, slug) = setup_bitmask_test(&app, "case5", config).await;
    let date = get_target_date(0);

    // Book 09:30-10:30
    let b_res = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug)).header("Content-Type", "application/json").body(Body::from(json!({"date": date, "time": "09:30", "name":"X", "email":"x@x.com"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(b_res.status(), StatusCode::OK);

    let res = app.router.clone().oneshot(Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date)).body(Body::empty()).unwrap()).await.unwrap();
    let slots = parse_body(res).await["slots"].as_array().unwrap().clone();

    // 09:00 should be gone because it would end at 10:00, overlapping the booking start at 09:30
    let has_0900 = slots.iter().any(|s| s.as_str().unwrap().contains("T09:00:00"));
    assert!(!has_0900);
}