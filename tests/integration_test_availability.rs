mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Utc, NaiveDate};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_tenant_and_event(app: &TestApp, slug_suffix: &str) -> (String, String, String) {
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": format!("Avail Test {}", slug_suffix), "slug": format!("avail-{}", slug_suffix)}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();

    let auth = app.login(&tid, "admin", &sec).await;

    let ev_payload = json!({
        "slug": format!("ev-{}", slug_suffix),
        "title_en": "E", "title_de": "E", "desc_en": ".", "desc_de": ".",
        "location": "HQ", "payout": "0", "host_name": "Host", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(60)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": { "monday": [{"start":"09:00","end":"12:00"}] },
        "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    (tid, sec, format!("ev-{}", slug_suffix))
}

fn next_monday_date() -> String {
    let mut next = Utc::now();
    while next.format("%A").to_string() != "Monday" { next += Duration::days(1); }
    if next < Utc::now() { next += Duration::days(7); }
    next.format("%Y-%m-%d").to_string()
}

#[tokio::test]
async fn test_standard_availability() {
    let app = TestApp::new().await;
    let (tid, _, slug) = create_tenant_and_event(&app, "std").await;
    let date = next_monday_date();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    assert_eq!(slots.len(), 3);
    // Slots are ISO strings. 09:00 UTC -> ...T09:00:00...
    assert!(slots[0].as_str().unwrap().contains("T09:00:00"));
    assert!(slots[2].as_str().unwrap().contains("T11:00:00"));
}

#[tokio::test]
async fn test_override_block_day() {
    let app = TestApp::new().await;
    let (tid, sec, slug) = create_tenant_and_event(&app, "block").await;
    let date = next_monday_date();

    let auth = app.login(&tid, "admin", &sec).await;

    let override_payload = json!({
        "date": date,
        "is_unavailable": true,
        "config": null,
        "location": null,
        "host_name": null
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();
    assert!(slots.is_empty(), "Slots should be empty for blocked day");
}

#[tokio::test]
async fn test_override_change_hours() {
    let app = TestApp::new().await;
    let (tid, sec, slug) = create_tenant_and_event(&app, "hours").await;
    let date = next_monday_date();

    let auth = app.login(&tid, "admin", &sec).await;

    let override_payload = json!({
        "date": date,
        "is_unavailable": false,
        "config": { "monday": [{"start": "13:00", "end": "15:00"}] },
        "location": null,
        "host_name": null
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    assert_eq!(slots.len(), 2);
    assert!(slots[0].as_str().unwrap().contains("T13:00:00"));
    assert!(slots[1].as_str().unwrap().contains("T14:00:00"));
}

#[tokio::test]
async fn test_override_location_fallback() {
    let app = TestApp::new().await;
    let (tid, sec, slug) = create_tenant_and_event(&app, "loc").await;
    let date = next_monday_date();

    let auth = app.login(&tid, "admin", &sec).await;

    let override_payload = json!({
        "date": date,
        "is_unavailable": false,
        "config": null,
        "location": "Beach",
        "host_name": null
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    assert_eq!(slots.len(), 3);
    assert!(slots[0].as_str().unwrap().contains("T09:00:00"));
}

#[tokio::test]
async fn test_slot_consumption() {
    let app = TestApp::new().await;
    let (tid, _, slug) = create_tenant_and_event(&app, "consume").await;
    let date = next_monday_date();

    // 1. Book 10:00
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"T", "email":"t@t.com"}).to_string())).unwrap()
    ).await.unwrap();

    // 2. Check Slots
    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    assert_eq!(slots.len(), 2);
    // 09:00 and 11:00 present, 10:00 gone
    assert!(slots.iter().any(|s| s.as_str().unwrap().contains("T09:00:00")));
    assert!(!slots.iter().any(|s| s.as_str().unwrap().contains("T10:00:00")));
}

#[tokio::test]
async fn test_available_dates_range() {
    let app = TestApp::new().await;
    let (tid, sec, slug) = create_tenant_and_event(&app, "range").await;
    let auth = app.login(&tid, "admin", &sec).await;

    let d1 = next_monday_date();
    let d1_parsed = NaiveDate::parse_from_str(&d1, "%Y-%m-%d").unwrap();
    let d2 = (d1_parsed + Duration::days(7)).format("%Y-%m-%d").to_string();

    let override_payload = json!({
        "date": d1,
        "is_unavailable": true,
        "config": null,
        "location": null, "host_name": null
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/overrides", tid, slug))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    let end_query = (d1_parsed + Duration::days(10)).format("%Y-%m-%d").to_string();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/dates?start={}&end={}", tid, slug, d1, end_query))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let dates = body.as_array().unwrap();

    let has_d1 = dates.contains(&json!(d1));
    let has_d2 = dates.contains(&json!(d2));

    assert!(!has_d1, "Blocked date should not be returned in available dates");
    assert!(has_d2, "Standard date should be returned");
}