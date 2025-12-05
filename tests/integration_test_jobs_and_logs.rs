mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::Utc;
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;
use booking_backend::domain::models::job::Job;
use booking_backend::domain::models::communication::MailLog;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_list_jobs() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Job Corp", "slug": "job-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &sec).await;

    // 2. Create dummy job manually in DB
    let _job_id = Uuid::new_v4().to_string();
    let job = Job::new("TEST_JOB", "dummy-booking-id".to_string(), tenant_id.clone(), Utc::now());
    app.state.job_repo.create(&job).await.unwrap();

    // 3. Call List Jobs Endpoint
    let res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/jobs", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = parse_body(res).await;
    let jobs = body.as_array().unwrap();

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["id"], job.id);
    assert_eq!(jobs[0]["job_type"], "TEST_JOB");
}

#[tokio::test]
async fn test_get_communication_logs() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Log Corp", "slug": "log-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &sec).await;

    // 2. Create Dummy Job & Log
    let _job_id = Uuid::new_v4().to_string();
    let job = Job::new("TEST_EMAIL", "dummy".to_string(), tenant_id.clone(), Utc::now());
    app.state.job_repo.create(&job).await.unwrap();

    let log = MailLog {
        id: Uuid::new_v4().to_string(),
        job_id: job.id.clone(),
        recipient: "test@example.com".to_string(),
        template_id: "Welcome Template".to_string(),
        context_hash: "hash123".to_string(),
        sent_at: Utc::now(),
        status: "SENT".to_string(),
    };
    app.state.communication_repo.log_mail(&log).await.unwrap();

    // 3. Fetch Logs (All)
    let res_all = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/communication/logs", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res_all.status(), StatusCode::OK);
    let logs_all = parse_body(res_all).await;
    assert_eq!(logs_all.as_array().unwrap().len(), 1);
    assert_eq!(logs_all[0]["recipient"], "test@example.com");

    // 4. Fetch Logs (Filtered by Recipient)
    let res_filter = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/communication/logs?recipient=test@example.com", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res_filter.status(), StatusCode::OK);
    let logs_filter = parse_body(res_filter).await;
    assert_eq!(logs_filter.as_array().unwrap().len(), 1);

    // 5. Fetch Logs (Filtered by Wrong Recipient)
    let res_empty = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/communication/logs?recipient=other@example.com", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let logs_empty = parse_body(res_empty).await;
    assert_eq!(logs_empty.as_array().unwrap().len(), 0);
}