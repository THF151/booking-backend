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
async fn test_booking_rejected_by_override_time_window() {
    let app = TestApp::new().await;

    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "name": "Constraint Corp",
                "slug": "constraint-corp"
            }).to_string())).unwrap()
    ).await.unwrap();

    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    let ev_payload = json!({
        "slug": "meet-constraint",
        "title_en": "M", "title_de": "M", "desc_en": ".", "desc_de": ".",
        "location": "HQ", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(60)).to_rfc3339(),
        "duration_min": 30,
        "interval_min": 30,
        "max_participants": 1,
        "image_url": ".",
        "config": {
            "monday": [{"start": "09:00", "end": "18:00"}]
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

    let mut next_mon = Utc::now();
    while next_mon.format("%A").to_string() != "Monday" {
        next_mon += Duration::days(1);
    }
    next_mon += Duration::days(7);
    let date_str = next_mon.format("%Y-%m-%d").to_string();

    let override_payload = json!({
        "date": date_str,
        "is_unavailable": false,
        "config": {
            "monday": [{"start": "09:00", "end": "15:00"}]
        },
        "location": null,
        "host_name": null
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/meet-constraint/overrides", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    let book_payload = json!({
        "date": date_str,
        "time": "17:00",
        "name": "Rule Breaker",
        "email": "break@rules.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/meet-constraint/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(book_payload.to_string())).unwrap()
    ).await.unwrap();

    assert_ne!(res.status(), StatusCode::OK);
}