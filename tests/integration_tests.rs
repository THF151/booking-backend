mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::TestApp;
use tower::ServiceExt; // for `oneshot`
use serde_json::{json, Value};
use chrono::{Utc, Duration};


// --- HAPPY PATH SCENARIOS ---

#[tokio::test]
async fn test_health_check() {
    let app = TestApp::new().await;

    let response = app.router.clone().oneshot(
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_full_saas_lifecycle() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let tenant_req = json!({ "name": "Test Corp", "slug": "test-corp" });
    let response = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/tenants")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&tenant_req).unwrap()))
            .unwrap(),
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let tenant_data: Value = serde_json::from_slice(&body_bytes).unwrap();

    let tenant_id = tenant_data["tenant_id"].as_str().unwrap();
    let admin_secret = tenant_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tenant_id, "admin", admin_secret).await;

    // 2. Create Event (OPEN access)
    let event_req = json!({
        "slug": "open-meeting",
        "title_en": "Open", "title_de": "Offen",
        "desc_en": "Desc", "desc_de": "Beschr",
        "location": "Zoom", "payout": "0",
        "host_name": "Test Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(365)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 10,
        "image_url": "http://img",
        "config": { "monday": [{"start": "09:00", "end": "17:00"}] },
        "access_mode": "OPEN"
    });

    let response = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&event_req).unwrap()))
            .unwrap(),
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 3. Book Appointment (Future Monday)
    let mut target_date = Utc::now();
    while target_date.format("%A").to_string() != "Monday" {
        target_date += Duration::days(1);
    }
    target_date += Duration::days(7);
    let date_str = target_date.format("%Y-%m-%d").to_string();

    let book_req = json!({
        "date": date_str,
        "time": "09:00",
        "name": "Tester",
        "email": "test@example.com"
    });

    let response = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/open-meeting/book", tenant_id))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&book_req).unwrap()))
            .unwrap(),
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// --- ERROR HANDLING & RESTRICTED FLOW SCENARIOS ---

#[tokio::test]
async fn test_restricted_flow_token_logic() {
    let app = TestApp::new().await;

    // Setup Tenant
    let tenant_req = json!({ "name": "Secure Corp", "slug": "secure-corp" });
    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&tenant_req).unwrap()))
            .unwrap()
    ).await.unwrap();
    let tenant_data: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    let tenant_id = tenant_data["tenant_id"].as_str().unwrap();
    let admin_secret = tenant_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tenant_id, "admin", admin_secret).await;

    // 1. Create RESTRICTED Event
    let event_req = json!({
        "slug": "vip-meeting",
        "title_en": "VIP", "title_de": "VIP", "desc_en": ".", "desc_de": ".", "location": ".", "payout": "0",
        "host_name": "Test Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(), "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": { "monday": [{"start": "09:00", "end": "12:00"}] },
        "access_mode": "RESTRICTED"
    });

    app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&event_req).unwrap()))
            .unwrap()
    ).await.unwrap();

    // 2. Try Booking WITHOUT Token (Should Fail 403)
    let book_req = json!({
        "date": "2025-01-01", "time": "09:00", "name": "Hacker", "email": "bad@guy.com"
    });

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-meeting/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&book_req).unwrap()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Create Token
    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-meeting/invitees", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({ "email": "vip@guy.com" }).to_string()))
            .unwrap()
    ).await.unwrap();
    let token_data: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    let token = token_data["token"].as_str().unwrap();

    // 4. Book WITH Token (Future Date)
    let mut future_mon = Utc::now();
    while future_mon.format("%A").to_string() != "Monday" { future_mon += Duration::days(1); }
    future_mon += Duration::days(14); // 2 weeks out

    let book_valid = json!({
        "date": future_mon.format("%Y-%m-%d").to_string(),
        "time": "09:00",
        "name": "VIP", "email": "vip@guy.com",
        "token": token
    });

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-meeting/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&book_valid).unwrap()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Try Book AGAIN with USED Token (Should Fail 409 Conflict)
    let book_again = json!({
        "date": future_mon.format("%Y-%m-%d").to_string(),
        "time": "10:00",
        "name": "VIP 2", "email": "vip@guy.com",
        "token": token
    });

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-meeting/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&book_again).unwrap()))
            .unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_validation_errors() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let tenant_req = json!({ "name": "Val Corp", "slug": "val-corp" });
    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&tenant_req).unwrap())).unwrap()
    ).await.unwrap();
    let tenant_data: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    let tenant_id = tenant_data["tenant_id"].as_str().unwrap();
    let admin_secret = tenant_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tenant_id, "admin", admin_secret).await;

    // 2. Try Create Event with End Date < Start Date
    let invalid_dates = json!({
        "slug": "bad-date", "title_en": "Bad", "title_de": "Bad", "desc_en": ".", "desc_de": ".", "location": ".", "payout": "0",
        "host_name": "Test Host",
        "timezone": "UTC",
        "active_start": "2025-01-01T00:00:00Z",
        "active_end": "2024-01-01T00:00:00Z",
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": {}, "access_mode": "OPEN"
    });

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&invalid_dates).unwrap()))
            .unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 3. Try Create Event with Duplicate Slug (DB Constraint)
    let valid_event = json!({
        "slug": "unique", "title_en": "Bad", "title_de": "Bad", "desc_en": ".", "desc_de": ".", "location": ".", "payout": "0",
        "host_name": "Test Host",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(), "active_end": (Utc::now() + Duration::days(1)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": {}, "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&valid_event).unwrap())).unwrap()
    ).await.unwrap();

    // Second: Duplicate
    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&valid_event).unwrap())).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::CONFLICT);
}