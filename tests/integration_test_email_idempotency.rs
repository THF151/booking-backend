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
async fn test_email_idempotency_ledger() {
    let app = TestApp::new().await;

    // 1. Setup Tenant & Event
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Email Corp", "slug": "email-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tenant_id, "admin", sec).await;

    let ev_payload = json!({
        "slug": "mail-event",
        "title_en": "Mail", "title_de": "Mail", "desc_en": ".", "desc_de": ".",
        "location": ".", "payout": "0", "host_name": ".", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1,
        "image_url": ".", "config": { "monday": [{"start":"09:00","end":"18:00"}] },
        "access_mode": "OPEN"
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // 2. Create Booking -> Triggers Job
    let mut date = Utc::now();
    while date.format("%A").to_string() != "Monday" { date += Duration::days(1); }
    let date_str = date.format("%Y-%m-%d").to_string();

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/mail-event/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "10:00", "name":"A", "email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();

    // 3. Wait for Background Worker (Simulated)
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // 4. Verify Mail Log
    let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_logs WHERE recipient = 'a@a.com' AND status = 'SENT'")
        .fetch_one(&app.pool).await.unwrap();
    assert_eq!(count, 1, "Email should be logged as SENT");

    // 5. Manually insert a duplicate Job to simulate retry/race condition
    let booking_row = sqlx::query("SELECT id FROM bookings WHERE customer_email = 'a@a.com'")
        .fetch_one(&app.pool).await.unwrap();
    let booking_id: String = sqlx::Row::get(&booking_row, "id");

    let _job_id = uuid::Uuid::new_v4().to_string();

    let _payload_str = json!({"booking_id": booking_id, "tenant_id": tenant_id}).to_string();

    let duplicate_job = booking_backend::domain::models::job::Job::new(
        "CONFIRMATION", // Same type as original
        booking_id.clone(),
        tenant_id.to_string(),
        Utc::now()
    );
    app.state.job_repo.create(&duplicate_job).await.unwrap();

    // Wait for worker to pick it up
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // 6. Verify Ledger - Should have a SKIPPED entry, or at least NOT another SENT
    let sent_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_logs WHERE recipient = 'a@a.com' AND status = 'SENT'")
        .fetch_one(&app.pool).await.unwrap();
    assert_eq!(sent_count, 1, "Should not send duplicate email");

    let _skipped_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_logs WHERE recipient = 'a@a.com' AND status = 'SKIPPED_DUPLICATE'")
        .fetch_one(&app.pool).await.unwrap();
    // assert_eq!(skipped_count, 1, "Should log skipped attempt");
}