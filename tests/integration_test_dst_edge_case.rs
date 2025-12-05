mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_dst_spring_forward_gap() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "DST Corp", "slug": "dst-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    // 2. Create Event (Open 24h, valid far into future)
    let active_start = Utc::now();
    let active_end = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

    let ev_payload = json!({
        "slug": "dst-event",
        "title_en": "DST Check", "title_de": "DST", "desc_en": ".", "desc_de": ".",
        "location": "Berlin", "payout": "0", "host_name": "Host",
        "timezone": "Europe/Berlin",
        "active_start": active_start.to_rfc3339(),
        "active_end": active_end.to_rfc3339(),
        "duration_min": 30,
        "interval_min": 30,
        "max_participants": 1,
        "image_url": ".",
        "config": {
            "sunday": [{"start":"00:00","end":"23:59"}]
        },
        "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // --- Scenario: Future DST Spring Forward ---
    // Berlin/CET Spring Forward: March 29, 2026.
    // 02:00 Local -> 03:00 Local.
    // 02:00 Local does not exist.
    // Slots are 30min interval.
    // 01:30 Local = 00:30 UTC.
    // 02:00 Local (Skipped).
    // 03:00 Local = 01:00 UTC.

    let date_dst = "2026-03-29";

    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/dst-event/slots?date={}", tid, date_dst))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // 1. Ensure 01:30 Local exists (00:30 UTC)
    let has_00_30_utc = slots.iter().any(|s| s.as_str().unwrap().contains("T00:30:00"));
    assert!(has_00_30_utc, "01:30 Local (00:30 UTC) should exist");

    // 2. Ensure 03:00 Local exists (01:00 UTC)
    let has_01_00_utc = slots.iter().any(|s| s.as_str().unwrap().contains("T01:00:00"));
    assert!(has_01_00_utc, "03:00 Local (01:00 UTC) should exist");

    assert!(!slots.is_empty());
}