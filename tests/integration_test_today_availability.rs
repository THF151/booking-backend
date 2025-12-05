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
async fn test_today_availability_logic() {
    let app = TestApp::new().await;

    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Today Corp", "slug": "today-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    let ev_payload = json!({
        "slug": "late-night",
        "title_en": "Late Night", "title_de": "Spät", "desc_en": ".", "desc_de": ".",
        "location": "Bar", "payout": "0", "host_name": "H",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(5)).to_rfc3339(),
        "duration_min": 30,
        "interval_min": 30,
        "max_participants": 1,
        "image_url": ".",
        "config": {
            "monday": [{"start":"08:00","end":"23:00"}],
            "tuesday": [{"start":"08:00","end":"23:00"}],
            "wednesday": [{"start":"08:00","end":"23:00"}],
            "thursday": [{"start":"08:00","end":"23:00"}],
            "friday": [{"start":"08:00","end":"23:00"}],
            "saturday": [{"start":"08:00","end":"23:00"}],
            "sunday": [{"start":"08:00","end":"23:00"}]
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

    let today_str = Utc::now().format("%Y-%m-%d").to_string();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/late-night/slots?date={}", tid, today_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    if Utc::now().format("%H").to_string() < "22".to_string() {
        assert!(!slots.is_empty());
    }
}
