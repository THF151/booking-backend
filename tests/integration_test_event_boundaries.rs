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

#[tokio::test]
async fn test_event_start_end_time_boundaries() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "name": "Boundary Corp",
                "slug": "boundary-corp"
            }).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();

    let auth = app.login(&tid, "admin", &sec).await;

    // Define Dates (Next Wednesday)
    let mut start_date = Utc::now();
    while start_date.format("%A").to_string() != "Wednesday" {
        start_date += Duration::days(1);
    }
    start_date += Duration::days(7);

    // Event Active Start: 14:00 UTC
    let active_start = start_date.date_naive().and_hms_opt(14, 0, 0).unwrap().and_utc();
    // Event Active End: 18:00 UTC
    let active_end = start_date.date_naive().and_hms_opt(18, 0, 0).unwrap().and_utc();

    let date_str = active_start.format("%Y-%m-%d").to_string();

    // Config: Open 09:00 to 20:00 in UTC
    let day_key = active_start.format("%A").to_string().to_lowercase();
    let mut config_map = serde_json::Map::new();
    config_map.insert(day_key, json!([{"start": "09:00", "end": "20:00"}]));

    let ev_payload = json!({
        "slug": "boundary-test",
        "title_en": "B", "title_de": "B", "desc_en": ".", "desc_de": ".",
        "location": "HQ", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": active_start.to_rfc3339(),
        "active_end": active_end.to_rfc3339(),
        "duration_min": 60,
        "interval_min": 60,
        "max_participants": 1,
        "image_url": ".",
        "config": Value::Object(config_map),
        "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // 4. Fetch Slots
    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/boundary-test/slots?date={}", tid, date_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // 09:00 - 13:00 should be hidden (Before active_start 14:00)
    // 14:00 should be First
    let first_slot = slots.first().expect("Should have slots").as_str().unwrap();
    assert!(first_slot.contains("T14:00:00"));

    // 18:00 is active_end.
    // Slot 17:00 -> Ends 18:00. Valid.
    // Slot 18:00 -> Ends 19:00. Invalid.
    let last_slot = slots.last().expect("Should have slots").as_str().unwrap();
    assert!(last_slot.contains("T17:00:00"));
}