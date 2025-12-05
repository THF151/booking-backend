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

async fn create_setup(app: &TestApp, slug: &str, access_mode: &str) -> (String, String, String) {
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Test", "slug": slug}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();

    let auth = app.login(&tid, "admin", &sec).await;

    let ev_slug = "ev1".to_string();
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug": ev_slug, "title_en": "E", "title_de": "E", "desc_en": ".", "desc_de": ".",
                "location": "Loc", "payout": "0", "host_name": "H", "timezone": "UTC",
                "active_start": Utc::now().to_rfc3339(),
                "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
                "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
                "config": { "monday": [{"start":"08:00", "end":"18:00"}] },
                "access_mode": access_mode
            }).to_string())).unwrap()
    ).await.unwrap();

    (tid, sec, ev_slug)
}

fn next_monday() -> String {
    let mut next = Utc::now();
    while next.format("%A").to_string() != "Monday" { next += Duration::days(1); }
    if next < Utc::now() { next += Duration::days(7); }
    next.format("%Y-%m-%d").to_string()
}

#[tokio::test]
async fn test_booking_with_note() {
    let app = TestApp::new().await;
    let (tid, sec, ev_slug) = create_setup(&app, "t1", "OPEN").await;
    let date = next_monday();

    let auth = app.login(&tid, "admin", &sec).await;

    let payload = json!({
        "date": date, "time": "10:00",
        "name": "Alice", "email": "a@a.com",
        "notes": "Vegan meal please"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    assert_eq!(body["customer_note"], "Vegan meal please");

    let list_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/bookings", tid, ev_slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let list = parse_body(list_res).await;
    assert_eq!(list[0]["customer_note"], "Vegan meal please");
}

#[tokio::test]
async fn test_booking_without_note() {
    let app = TestApp::new().await;
    let (tid, _, ev_slug) = create_setup(&app, "t2", "OPEN").await;
    let date = next_monday();

    let payload = json!({
        "date": date, "time": "11:00",
        "name": "Bob", "email": "b@b.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    assert!(body["customer_note"].is_null());
}

#[tokio::test]
async fn test_booking_removes_slot() {
    let app = TestApp::new().await;
    let (tid, _, ev_slug) = create_setup(&app, "t3", "OPEN").await;
    let date = next_monday();

    // 1. Book 12:00
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "12:00", "name": "C", "email": "c@c.com"}).to_string())).unwrap()
    ).await.unwrap();

    // 2. Get Slots
    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, ev_slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // 12:00 should be missing.
    let has_12 = slots.iter().any(|s| s.as_str().unwrap().contains("T12:00:00"));
    assert!(!has_12, "Slot 12:00 should be taken");

    // 13:00 should be present
    let has_13 = slots.iter().any(|s| s.as_str().unwrap().contains("T13:00:00"));
    assert!(has_13, "Slot 13:00 should be available");
}

#[tokio::test]
async fn test_double_booking_conflict() {
    let app = TestApp::new().await;
    let (tid, _, ev_slug) = create_setup(&app, "t4", "OPEN").await;
    let date = next_monday();

    let payload = json!({ "date": date, "time": "14:00", "name": "D", "email": "d@d.com" }).to_string();

    let r1 = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug)).header("Content-Type","application/json").body(Body::from(payload.clone())).unwrap()).await.unwrap();
    assert_eq!(r1.status(), StatusCode::OK);

    let r2 = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug)).header("Content-Type","application/json").body(Body::from(payload)).unwrap()).await.unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_restricted_event_token_flow() {
    let app = TestApp::new().await;
    let (tid, sec, ev_slug) = create_setup(&app, "t5", "RESTRICTED").await;
    let date = next_monday();
    let auth = app.login(&tid, "admin", &sec).await;

    // 1. Try Book without Token -> Forbidden
    let fail = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "09:00", "name":"X", "email":"x@x.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(fail.status(), StatusCode::FORBIDDEN);

    // 2. Create Token
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/invitees", tid, ev_slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"email": "vip@vip.com"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let token = t_data["token"].as_str().unwrap();

    // 3. Book WITH Token -> OK
    let success = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date, "time": "09:00", "name":"VIP", "email":"vip@vip.com",
                "token": token
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(success.status(), StatusCode::OK);

    // 4. Reuse Token -> CONFLICT
    let reuse = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date, "time": "10:00", "name":"VIP2", "email":"vip@vip.com",
                "token": token
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(reuse.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_booking() {
    let app = TestApp::new().await;
    let (tid, sec, ev_slug) = create_setup(&app, "del-book", "OPEN").await;
    let date = next_monday();
    let auth = app.login(&tid, "admin", &sec).await;

    let create_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, ev_slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date, "time": "10:00", "name": "To Delete", "email": "del@del.com"
            }).to_string())).unwrap()
    ).await.unwrap();

    let create_data = parse_body(create_res).await;
    let booking_id = create_data["id"].as_str().unwrap();

    let del_res = app.router.clone().oneshot(
        Request::builder().method("DELETE").uri(format!("/api/v1/{}/bookings/{}", tid, booking_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(del_res.status(), StatusCode::OK);

    let get_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/bookings/{}", tid, booking_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(get_res.status(), StatusCode::NOT_FOUND);
}