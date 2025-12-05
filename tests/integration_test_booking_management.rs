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
async fn test_booking_cancellation_and_rescheduling() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Mgmt Corp", "slug": "mgmt-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tid, "admin", &sec).await;

    // 2. Create Event
    let ev_payload = json!({
        "slug": "mgmt-event",
        "title_en": "Mgmt", "title_de": "Mgmt", "desc_en": ".", "desc_de": ".",
        "location": "Room 1", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1,
        "image_url": ".", "config": { "monday": [{"start":"09:00","end":"17:00"}] },
        "access_mode": "OPEN",
        "allow_customer_cancel": true,
        "allow_customer_reschedule": true
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // Find Next Monday
    let mut date = Utc::now();
    while date.format("%A").to_string() != "Monday" { date += Duration::days(1); }
    date += Duration::days(7);
    let date_str = date.format("%Y-%m-%d").to_string();

    // 3. Create Booking
    let b_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/mgmt-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "10:00", "name":"A", "email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    let booking = parse_body(b_res).await;
    let token = booking["management_token"].as_str().unwrap();

    // 4. Cancel Booking
    let cancel_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/bookings/manage/{}/cancel", token))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(cancel_res.status(), StatusCode::OK);

    // Verify Slot 10:00 is available again
    let slots_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/mgmt-event/slots?date={}", tid, date_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots = parse_body(slots_res).await["slots"].as_array().unwrap().clone();
    assert!(slots.iter().any(|s| s.as_str().unwrap().contains("T10:00:00")));

    // 5. Create another booking to test Reschedule
    let b2_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/mgmt-event/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "10:00", "name":"B", "email":"b@b.com"}).to_string())).unwrap()
    ).await.unwrap();
    let booking2 = parse_body(b2_res).await;
    let token2 = booking2["management_token"].as_str().unwrap();

    // 6. Reschedule B from 10:00 to 12:00
    let resched_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/bookings/manage/{}/reschedule", token2))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "12:00"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(resched_res.status(), StatusCode::OK);

    // Verify 10:00 is free, 12:00 is taken
    let slots_res_2 = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/mgmt-event/slots?date={}", tid, date_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let slots_2 = parse_body(slots_res_2).await["slots"].as_array().unwrap().clone();
    assert!(slots_2.iter().any(|s| s.as_str().unwrap().contains("T10:00:00")));
    assert!(!slots_2.iter().any(|s| s.as_str().unwrap().contains("T12:00:00")));
}