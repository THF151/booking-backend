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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_tenant_creation_and_auth() {
    let app = TestApp::new().await;

    let payload = json!({
        "name": "Acme Corp",
        "slug": "acme-corp"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/tenants")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let response = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_body(response).await;
    let tenant_id = body["tenant_id"].as_str().unwrap();
    let admin_secret = body["admin_secret"].as_str().unwrap();

    assert!(!tenant_id.is_empty());
    assert!(!admin_secret.is_empty());

    let auth = app.login(tenant_id, "admin", admin_secret).await;
    assert!(!auth.access_token.is_empty());
    assert!(!auth.csrf_token.is_empty());
}

#[tokio::test]
async fn test_event_lifecycle_open_access() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let tenant_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Open Corp", "slug": "open"}).to_string())).unwrap(),
    ).await.unwrap();
    let tenant_data = parse_body(tenant_res).await;
    let tenant_id = tenant_data["tenant_id"].as_str().unwrap();
    let admin_secret = tenant_data["admin_secret"].as_str().unwrap();

    // Login
    let auth = app.login(tenant_id, "admin", admin_secret).await;

    // 2. Create Event
    let event_payload = json!({
        "slug": "coffee-chat",
        "title_en": "Coffee", "title_de": "Kaffee",
        "desc_en": "Chat", "desc_de": "Reden",
        "location": "Online", "payout": "0",
        "host_name": "Dr. Barista",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 30, "interval_min": 30, "max_participants": 1,
        "image_url": "http://img.com",
        "config": { "monday": [{"start": "09:00", "end": "12:00"}] },
        "access_mode": "OPEN"
    });

    let event_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(event_payload.to_string())).unwrap(),
    ).await.unwrap();
    assert_eq!(event_res.status(), StatusCode::OK);

    // 3. Find next Monday
    let mut next_monday = Utc::now();
    while next_monday.format("%A").to_string() != "Monday" {
        next_monday += Duration::days(1);
    }
    next_monday += Duration::days(7);
    let date_str = next_monday.format("%Y-%m-%d").to_string();

    // 4. Get Slots
    let slots_res = app.router.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/{}/events/coffee-chat/slots?date={}", tenant_id, date_str))
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(slots_res.status(), StatusCode::OK);
    let slots_data = parse_body(slots_res).await;
    let slots = slots_data["slots"].as_array().unwrap();
    assert!(!slots.is_empty());

    let first_slot = slots[0].as_str().unwrap(); // e.g. "2025-XX-XXT09:00:00+00:00"

    let book_payload = json!({
        "date": date_str,
        "time": "09:00",
        "name": "John Doe",
        "email": "john@example.com"
    });

    let book_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/coffee-chat/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(book_payload.to_string())).unwrap(),
    ).await.unwrap();

    if book_res.status() != StatusCode::OK {
        let body = parse_body(book_res).await;
        println!("Booking Failed: {:?}", body);
        panic!("Booking failed");
    }

    // 6. Verify Slot Gone
    let slots_res_2 = app.router.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/{}/events/coffee-chat/slots?date={}", tenant_id, date_str))
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    let slots_data_2 = parse_body(slots_res_2).await;
    let slots_2 = slots_data_2["slots"].as_array().unwrap();
    assert!(!slots_2.contains(&json!(first_slot)));
}

#[tokio::test]
async fn test_restricted_event_token_usage_and_duplication() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let tenant_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Secure", "slug": "sec"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(tenant_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    // 2. Create RESTRICTED Event
    let event_payload = json!({
        "slug": "vip-session",
        "title_en": "VIP", "title_de": "VIP", "desc_en": ".", "desc_de": ".", "location": ".", "payout": "0",
        "host_name": "Dr. VIP",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(60)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": { "tuesday": [{"start": "10:00", "end": "12:00"}] },
        "access_mode": "RESTRICTED"
    });

    let ev_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(event_payload.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(ev_res.status(), StatusCode::OK);

    // 3. Attempt Booking WITHOUT Token -> 403 Forbidden
    let mut next_tue = Utc::now();
    while next_tue.format("%A").to_string() != "Tuesday" { next_tue += Duration::days(1); }
    next_tue += Duration::days(7);
    let date_str = next_tue.format("%Y-%m-%d").to_string();

    let fail_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-session/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date_str, "time": "10:00", "name": "Hacker", "email": "h@h.com"
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(fail_res.status(), StatusCode::FORBIDDEN);

    // 4. Generate Token (Admin only)
    let token_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-session/invitees", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"email": "vip@client.com"}).to_string())).unwrap()
    ).await.unwrap();
    let token_data = parse_body(token_res).await;
    let token = token_data["token"].as_str().unwrap();

    // 5. Book WITH Token -> 200 OK
    let success_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-session/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date_str, "time": "10:00", "name": "VIP", "email": "vip@client.com", "token": token
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(success_res.status(), StatusCode::OK);

    // 6. Attempt Reuse of Token -> 409 Conflict
    let reuse_res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events/vip-session/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date_str, "time": "11:00", "name": "VIP", "email": "vip@client.com", "token": token
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(reuse_res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_invalid_inputs_and_logic() {
    let app = TestApp::new().await;

    // Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Err Corp", "slug": "err"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    // 1. Create Event with End before Start
    let bad_event = json!({
        "slug": "bad-time", "title_en": "A", "title_de": "A", "desc_en": "A", "desc_de": "A",
        "location": "A", "payout": "0", "duration_min": 60, "interval_min": 60, "max_participants": 1,
        "host_name": "Test Host", "timezone": "UTC",
        "image_url": "A", "config": {}, "access_mode": "OPEN",
        "active_start": "2025-01-02T00:00:00Z",
        "active_end": "2025-01-01T00:00:00Z"
    });

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(bad_event.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 2. Create Valid Event
    let good_event = json!({
        "slug": "good-time", "title_en": "A", "title_de": "A", "desc_en": "A", "desc_de": "A",
        "location": "A", "payout": "0", "duration_min": 60, "interval_min": 60, "max_participants": 1,
        "host_name": "Test Host", "timezone": "UTC",
        "image_url": "A", "config": { "friday": [{"start": "12:00", "end": "13:00"}] }, "access_mode": "OPEN",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(5)).to_rfc3339()
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(good_event.to_string())).unwrap()
    ).await.unwrap();

    // 3. Try booking outside active range (Past or far future)
    let past_date = (Utc::now() - Duration::days(50)).format("%Y-%m-%d").to_string();
    let res_past = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/good-time/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": past_date, "time": "12:00", "name": "Time Traveler", "email": "t@t.com"
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(res_past.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_auth_errors() {
    let app = TestApp::new().await;

    // 1. Create a Valid Tenant first
    let tenant_req = json!({ "name": "Auth Corp", "slug": "auth-corp" });
    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&tenant_req).unwrap())).unwrap()
    ).await.unwrap();
    let tenant_data = parse_body(res).await;
    let tenant_id = tenant_data["tenant_id"].as_str().unwrap();

    // 2. Try Create Event without Cookie (Should be 401)
    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from("{}")).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 3. Try Create Event with Cookie but missing CSRF (Should be 403)
    let secret = tenant_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tenant_id, "admin", secret).await;

    let res = app.router.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            // Missing X-CSRF-Token header
            .header("Content-Type", "application/json")
            .body(Body::from("{}")).unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}