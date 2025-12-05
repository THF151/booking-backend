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
async fn test_manual_session_scheduling() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Manual Corp", "slug": "manual-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    // 2. Create MANUAL Event
    let ev_payload = json!({
        "slug": "session-event",
        "title_en": "Manual", "title_de": "Manuell", "desc_en": ".", "desc_de": ".",
        "location": "Room 1", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, // Ignored for manual
        "image_url": ".", "config": {},
        "access_mode": "OPEN",
        "schedule_type": "MANUAL"
    });

    let ev_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(ev_res.status(), StatusCode::OK);

    // 3. Create Sessions
    let date = (Utc::now() + Duration::days(1)).format("%Y-%m-%d").to_string();

    // Session 1: 10:00 - 11:00, Cap 2
    let s1_payload = json!({
        "date": date,
        "start_time": "10:00",
        "end_time": "11:00",
        "max_participants": 2,
        "location": "Room A"
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/session-event/sessions", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(s1_payload.to_string())).unwrap()
    ).await.unwrap();

    // Session 2: 14:00 - 15:00, Cap 1
    let s2_payload = json!({
        "date": date,
        "start_time": "14:00",
        "end_time": "15:00",
        "max_participants": 1,
        "location": "Room B"
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/session-event/sessions", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(s2_payload.to_string())).unwrap()
    ).await.unwrap();

    // 4. Get Slots - Should only return 10:00 and 14:00
    let slots_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/session-event/slots?date={}", tid, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots = parse_body(slots_res).await["slots"].as_array().unwrap().clone();
    assert_eq!(slots.len(), 2);
    assert!(slots[0].as_str().unwrap().contains("T10:00:00"));
    assert!(slots[1].as_str().unwrap().contains("T14:00:00"));

    // 5. Book Session 1 (Cap 2) - 1st booking
    let b1 = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/session-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"A", "email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b1.status(), StatusCode::OK);

    // 6. Book Session 1 - 2nd booking (Should work)
    let b2 = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/session-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"B", "email":"b@b.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b2.status(), StatusCode::OK);

    // 7. Book Session 1 - 3rd booking (Should fail - Full)
    let b3 = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/session-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date, "time": "10:00", "name":"C", "email":"c@c.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b3.status(), StatusCode::CONFLICT);

    // 8. Verify Slots - 10:00 should be gone
    let slots_res_2 = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/session-event/slots?date={}", tid, date))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots_2 = parse_body(slots_res_2).await["slots"].as_array().unwrap().clone();
    assert_eq!(slots_2.len(), 1);
    assert!(slots_2[0].as_str().unwrap().contains("T14:00:00"));
}