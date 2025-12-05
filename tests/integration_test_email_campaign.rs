mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;
use chrono::{Duration, Utc};
use booking_backend::domain::models::communication::{EmailTemplate, NotificationRule, EmailTemplateVersion};

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_custom_email_campaign_flow() {

    let app = TestApp::new().await;

    // 1. Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Camp Corp", "slug": "camp-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &sec).await;

    // 2. Setup Event
    let ev_payload = json!({
        "slug": "launch-party",
        "title_en": "Launch", "title_de": "Start", "desc_en": ".", "desc_de": ".",
        "location": ".", "payout": "0", "host_name": ".", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 10,
        "image_url": ".", "config": { "monday": [{"start":"09:00","end":"18:00"}] },
        "access_mode": "OPEN"
    });
    let ev_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();
    let ev_data = parse_body(ev_res).await;
    let event_id = ev_data["id"].as_str().unwrap().to_string();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let existing_rules = app.state.communication_repo.get_rules_by_event(&event_id).await.unwrap();
    for r in existing_rules {
        if r.trigger_type == "ON_BOOKING" {
            app.state.communication_repo.delete_rule(&r.id).await.unwrap();
        }
    }

    // 3. Create Custom Confirmation Template
    let template = EmailTemplate::new(
        tenant_id.clone(),
        Some(event_id.clone()),
        "VIP Confirmation".to_string(),
        "You are VIP!".to_string(),
        "<mjml><mj-body><mj-section><mj-column><mj-text>Welcome {{user_name}}</mj-text></mj-column></mj-section></mj-body></mjml>".to_string(),
        "mjml".to_string()
    );

    let saved_template = app.state.communication_repo.create_template(&template).await.unwrap();

    // Create initial version to satisfy integrity if needed, though simple create works
    let version = EmailTemplateVersion::new(
        saved_template.id.clone(),
        saved_template.subject_template.clone(),
        saved_template.body_template.clone()
    );
    app.state.communication_repo.create_template_version(&version).await.unwrap();

    println!("Created Template: {} ({})", saved_template.name, saved_template.id);

    // 4. Link Template to Event (Confirmation Rule)
    let rule = NotificationRule::new(
        tenant_id.clone(),
        Some(event_id.clone()),
        "ON_BOOKING".to_string(),
        saved_template.id.clone()
    );
    app.state.communication_repo.create_rule(&rule).await.unwrap();
    println!("Created Rule for ON_BOOKING -> Template {}", saved_template.id);

    // 5. User Books
    let mut date = Utc::now();
    while date.format("%A").to_string() != "Monday" { date += Duration::days(1); }
    let date_str = date.format("%Y-%m-%d").to_string();

    println!("Booking for date: {}", date_str);

    let book_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/launch-party/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "10:00", "name":"VIP User", "email":"vip@test.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(book_res.status(), StatusCode::OK);

    // Check if Job was created
    let pending_jobs = sqlx::query("SELECT count(*) as count FROM jobs").fetch_one(&app.pool).await.unwrap();
    let job_count: i32 = sqlx::Row::get(&pending_jobs, "count");
    println!("Pending/Total Jobs in DB: {}", job_count);

    // 6. Wait for Worker
    println!("Waiting for worker...");
    tokio::time::sleep(std::time::Duration::from_secs(6)).await; // Increased wait time

    // Debug: Dump logs
    let logs = sqlx::query("SELECT recipient, template_id, status FROM mail_logs")
        .fetch_all(&app.pool).await.unwrap();
    println!("Mail Logs found: {}", logs.len());
    for row in logs {
        let r: String = sqlx::Row::get(&row, "recipient");
        let t: String = sqlx::Row::get(&row, "template_id");
        let s: String = sqlx::Row::get(&row, "status");
        println!(" - {} | {} | {}", r, t, s);
    }

    // 7. Verify Mail Log showed the CUSTOM template was used
    let log: i32 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mail_logs WHERE recipient = 'vip@test.com' AND template_id = 'VIP Confirmation' AND status = 'SENT'"
    )
        .fetch_one(&app.pool).await.unwrap();

    assert_eq!(log, 1, "Should have sent email using custom template name");
}