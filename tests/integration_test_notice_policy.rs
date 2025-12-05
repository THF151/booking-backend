mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::{Duration, Timelike, Utc};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn setup_test(app: &TestApp, slug: &str, general_delay: i32, first_delay: i32) -> (String, String, String) {
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Notice Corp", "slug": format!("n-{}", slug)}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();

    let auth = app.login(&tid, "admin", &sec).await;

    let ev_payload = json!({
        "slug": slug,
        "title_en": "Notice Test", "title_de": "Test", "desc_en": ".", "desc_de": ".",
        "location": "HQ", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "min_notice_general": general_delay,
        "min_notice_first": first_delay,
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(365)).to_rfc3339(),
        "duration_min": 60,
        "interval_min": 60,
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

    (tid, sec, slug.to_string())
}

fn get_future_date(days: i64) -> String {
    (Utc::now() + Duration::days(days)).format("%Y-%m-%d").to_string()
}

#[tokio::test]
async fn test_empty_day_uses_first_notice() {
    let app = TestApp::new().await;
    let (tid, _, slug) = setup_test(&app, "lina", 60, 240).await;

    let target_date_str = get_future_date(1);

    let res = app.router.clone().oneshot(
        Request::builder().method("GET")
            .uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, target_date_str))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    if let Some(first_slot) = slots.first() {
        let first_slot_time = chrono::DateTime::parse_from_rfc3339(first_slot.as_str().unwrap())
            .unwrap().with_timezone(&Utc);
        let now = Utc::now();
        let diff = first_slot_time - now;
        assert!(diff.num_minutes() >= 239, "First slot too close: {} mins", diff.num_minutes());
    }
}

#[tokio::test]
async fn test_follow_up_uses_general_notice() {
    let app = TestApp::new().await;
    // General: 1h, First: 4h
    let (tid, _, slug) = setup_test(&app, "max", 60, 240).await;

    // Use Today. Book at +5h.
    let today = Utc::now();

    if (today + Duration::hours(7)).date_naive() != today.date_naive() {
        println!("Skipping test_follow_up_uses_general_notice due to end of day.");
        return;
    }

    let today_str = today.format("%Y-%m-%d").to_string();

    // 1. Book at +5h (valid > 4h)
    let booking_start = today + Duration::hours(5);
    // Round to start of hour for cleaner matching
    let booking_start = booking_start.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap();
    let booking_time_str = booking_start.format("%H:%M").to_string();

    let b_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/{}/book", tid, slug))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": today_str, "time": booking_time_str, "name":"A", "email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(b_res.status(), StatusCode::OK);

    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/slots?date={}", tid, slug, today_str)).body(Body::empty()).unwrap()
    ).await.unwrap();
    let body = parse_body(res).await;
    let slots = body["slots"].as_array().unwrap();

    // 2. Check Slot at +2h (Before booking). Should be blocked by First Notice (4h).
    let target_early = today + Duration::hours(2);
    let target_early = target_early.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap();
    let has_early = slots.iter().any(|s| s.as_str().unwrap().contains(&target_early.format("T%H:%M").to_string()));
    assert!(!has_early, "Slot at +2h should be blocked by First Notice");

    // 3. Check Slot at +7h (After booking). Should be allowed by General Notice (1h).
    let target_late = today + Duration::hours(7);
    let target_late = target_late.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap();

    // Debug output if failing
    if !slots.iter().any(|s| s.as_str().unwrap().contains(&target_late.format("T%H:%M").to_string())) {
        println!("Slots found: {:?}", slots);
        println!("Target late: {}", target_late);
    }

    let has_late = slots.iter().any(|s| s.as_str().unwrap().contains(&target_late.format("T%H:%M").to_string()));
    assert!(has_late, "Slot at +7h should be allowed (General Notice 1h)");
}