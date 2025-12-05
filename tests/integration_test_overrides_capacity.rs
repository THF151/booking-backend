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
async fn test_recurring_event_capacity_override() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Cap Corp", "slug": "cap-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    // 2. Create Event (Default Cap 1)
    let ev_payload = json!({
        "slug": "cap-event",
        "title_en": "Cap Test", "title_de": "Cap", "desc_en": ".", "desc_de": ".",
        "location": "Room", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1,
        "image_url": ".",
        "config": { "monday": [{"start": "10:00", "end": "12:00"}] },
        "access_mode": "OPEN",
        "schedule_type": "RECURRING"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // Find next Monday
    let mut next_mon = Utc::now();
    while next_mon.format("%A").to_string() != "Monday" { next_mon += Duration::days(1); }
    next_mon += Duration::days(7); // Next week to avoid notice period issues
    let date = next_mon.format("%Y-%m-%d").to_string();

    // 3. Book 1 slot (Default cap is 1, so slot should vanish)
    let b1 = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"A", "email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b1.status(), StatusCode::OK);

    let slots_res_1 = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/cap-event/slots?date={}", tid, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots_1 = parse_body(slots_res_1).await["slots"].as_array().unwrap().clone();
    // 10:00 should be gone, 11:00 remains
    assert!(!slots_1.iter().any(|s| s.as_str().unwrap().contains("T10:00:00")));

    // 4. Create Override for that day -> Increase Capacity to 3
    let override_payload = json!({
        "date": date,
        "is_unavailable": false,
        "override_max_participants": 3
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-event/overrides", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(override_payload.to_string())).unwrap()
    ).await.unwrap();

    // 5. Verify Slot 10:00 reappears (1 booking < 3 cap)
    let slots_res_2 = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/cap-event/slots?date={}", tid, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots_2 = parse_body(slots_res_2).await["slots"].as_array().unwrap().clone();
    assert!(slots_2.iter().any(|s| s.as_str().unwrap().contains("T10:00:00")));

    // 6. Book again
    let b2 = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"B", "email":"b@b.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b2.status(), StatusCode::OK);
}
