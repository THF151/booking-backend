mod common;

use axum::{
    body::Body,
    http::{header, Request},
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
async fn test_specific_start_boundary_logic() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Boundary Corp", "slug": "boundary-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    // Simulate start time relative to now
    let now = Utc::now();
    let active_start = now + Duration::minutes(30);
    let active_end = active_start + Duration::days(365);

    let ev_payload = json!({
        "slug": "boundary-event",
        "title_en": "Boundary", "title_de": "Grenze", "desc_en": ".", "desc_de": ".",
        "location": "Online", "payout": "0", "host_name": "Bot",
        "timezone": "UTC",
        "active_start": active_start.to_rfc3339(),
        "active_end": active_end.to_rfc3339(),
        "duration_min": 10,
        "interval_min": 10,
        "max_participants": 1,
        "image_url": ".",
        "config": {
            "monday": [{"start": "00:00", "end": "23:59"}],
            "tuesday": [{"start": "00:00", "end": "23:59"}],
            "wednesday": [{"start": "00:00", "end": "23:59"}],
            "thursday": [{"start": "00:00", "end": "23:59"}],
            "friday": [{"start": "00:00", "end": "23:59"}],
            "saturday": [{"start": "00:00", "end": "23:59"}],
            "sunday": [{"start": "00:00", "end": "23:59"}]
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

    let today_str = now.format("%Y-%m-%d").to_string();
    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/boundary-event/slots?date={}", tid, today_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // Ensure no slots before active_start
    for slot in slots {
        let slot_dt = chrono::DateTime::parse_from_rfc3339(slot.as_str().unwrap()).unwrap().with_timezone(&Utc);
        assert!(slot_dt >= active_start);
    }
}