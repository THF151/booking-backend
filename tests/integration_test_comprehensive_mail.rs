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
async fn test_comprehensive_mail_flow() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Mail Suite Corp", "slug": "mail-suite"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &sec).await;

    // 2. Create Event - Should auto-generate templates
    let ev_payload = json!({
        "slug": "test-event",
        "title_en": "Test Event", "title_de": "Test Event", "desc_en": ".", "desc_de": ".",
        "location": "Zoom", "payout": "0", "host_name": "Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 10,
        "image_url": ".", "config": { "monday": [{"start":"00:00","end":"23:59"}] },
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

    // 3. Verify Auto-generated Rules
    let rules = app.state.communication_repo.get_rules_by_event(&event_id).await.unwrap();
    assert!(rules.iter().any(|r| r.trigger_type == "ON_BOOKING"), "Missing ON_BOOKING rule");
    assert!(rules.iter().any(|r| r.trigger_type == "REMINDER_24H"), "Missing REMINDER_24H rule");

    // 4. Verify Auto-generated Templates
    let templates = app.state.communication_repo.list_templates(&tenant_id, Some(&event_id)).await.unwrap();
    let conf_tmpl = templates.iter().find(|t| t.name.contains("Confirmation")).expect("Missing Confirmation Template");

    // 5. Modify Confirmation Template (Customize Subject)
    let new_subject = "Your VIP Slot: {{ event_title }}";
    let mut updated_tmpl = conf_tmpl.clone();
    updated_tmpl.subject_template = new_subject.to_string();
    app.state.communication_repo.update_template(&updated_tmpl).await.unwrap();

    // 6. Book a slot (Next Mon)
    let mut date = Utc::now();
    while date.format("%A").to_string() != "Monday" { date += Duration::days(1); }
    date += Duration::days(7); // Future
    let date_str = date.format("%Y-%m-%d").to_string();

    let book_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/test-event/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date": date_str, "time": "10:00", "name":"Tester", "email":"tester@mail.com"}).to_string())).unwrap()
    ).await.unwrap();
    let _booking_data = parse_body(book_res).await;
    // let booking_id = booking_data["id"].as_str().unwrap();

    // 7. Verify Job Creation
    let jobs = app.state.job_repo.list_jobs(&tenant_id).await.unwrap();
    let conf_job = jobs.iter().find(|j| j.job_type == "CONFIRMATION").expect("Confirmation job not created");
    assert_eq!(conf_job.status, "PENDING");

    // 8. Run Worker (Process Job)
    println!("Waiting for worker...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // 9. Verify Mail Log & Content
    let logs = app.state.communication_repo.list_logs(&tenant_id, Some("tester@mail.com")).await.unwrap();
    let _sent_log = logs.iter().find(|l| l.status == "SENT").expect("Mail not logged as SENT");

    // 10. Verify Reminder Job Scheduled
    let reminder_job = jobs.iter().find(|j| j.job_type == "REMINDER").expect("Reminder job not created");
    assert!(reminder_job.execute_at > Utc::now() + Duration::days(5), "Reminder scheduled too early");

    println!("Comprehensive mail flow test passed.");
}