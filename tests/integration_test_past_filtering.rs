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
async fn test_past_slots_are_filtered_concretely() {
    let app = TestApp::new().await;

    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Past Filter Corp", "slug": "past-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    let active_start = Utc::now() - Duration::days(1);
    let active_end = Utc::now() + Duration::days(1);

    let ev_payload = json!({
        "slug": "real-time-check",
        "title_en": "RT Check", "title_de": "Echtzeit",
        "desc_en": ".", "desc_de": ".",
        "location": "Online", "payout": "0", "host_name": "Bot",
        "timezone": "UTC",
        "active_start": active_start.to_rfc3339(),
        "active_end": active_end.to_rfc3339(),
        "duration_min": 15,
        "interval_min": 15,
        "max_participants": 1,
        "image_url": ".",
        "config": {
            "monday": [{"start":"00:00","end":"23:59"}],
            "tuesday": [{"start":"00:00","end":"23:59"}],
            "wednesday": [{"start":"00:00","end":"23:59"}],
            "thursday": [{"start":"00:00","end":"23:59"}],
            "friday": [{"start":"00:00","end":"23:59"}],
            "saturday": [{"start":"00:00","end":"23:59"}],
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

    let now_local = Utc::now();
    let today_str = now_local.format("%Y-%m-%d").to_string();

    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/real-time-check/slots?date={}", tid, today_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    let now_with_buffer = now_local - Duration::seconds(5);

    for slot_val in slots {
        let slot_str = slot_val.as_str().unwrap(); // "ISO String"
        let slot_dt = chrono::DateTime::parse_from_rfc3339(slot_str).unwrap().with_timezone(&Utc);

        if slot_dt < now_with_buffer {
            panic!("Past slot returned: {}", slot_dt);
        }
    }
}
