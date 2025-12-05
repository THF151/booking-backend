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
async fn test_event_update() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"U","slug":"up"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    let ev_load = json!({
        "slug": "orig", "title_en": "Orig", "title_de": "Orig", "desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(), "active_end": (Utc::now()+Duration::days(10)).to_rfc3339(),
        "duration_min":30,"interval_min":30,"max_participants":1,"image_url":".","config":{},"access_mode":"OPEN"
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_load.to_string())).unwrap()
    ).await.unwrap();

    let update_load = json!({ "host_name": "Updated Host", "title_en": "New Title" });
    let up_res = app.router.clone().oneshot(
        Request::builder().method("PUT").uri(format!("/api/v1/{}/events/orig", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(update_load.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(up_res.status(), StatusCode::OK);
    let up_data = parse_body(up_res).await;
    assert_eq!(up_data["host_name"], "Updated Host");
}

#[tokio::test]
async fn test_event_deletion() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"D","slug":"del"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug":"del-me","title_en":"D","title_de":"D","desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
                "timezone": "UTC",
                "active_start":Utc::now().to_rfc3339(),"active_end":(Utc::now()+Duration::days(1)).to_rfc3339(),
                "duration_min":30,"interval_min":30,"max_participants":1,"image_url":".","config":{},"access_mode":"OPEN"
            }).to_string())).unwrap()
    ).await.unwrap();

    let del_res = app.router.clone().oneshot(
        Request::builder().method("DELETE").uri(format!("/api/v1/{}/events/del-me", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(del_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_closed_event_access() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"C","slug":"clo"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    let ev_load = json!({
        "slug": "closed", "title_en": "C", "title_de": "C", "desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
        "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(), "active_end": (Utc::now()+Duration::days(10)).to_rfc3339(),
        "duration_min":30,"interval_min":30,"max_participants":1,"image_url":".","config":{},"access_mode":"CLOSED"
    });
    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_load.to_string())).unwrap()
    ).await.unwrap();

    let book_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/closed/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date":"2025-01-01","time":"10:00","name":"A","email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(book_res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_token_revocation() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"R","slug":"rev"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();

    let auth = app.login(tid, "admin", sec).await;

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug":"revo","title_en":"R","title_de":"R","desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
                "timezone": "UTC",
                "active_start":Utc::now().to_rfc3339(),"active_end":(Utc::now()+Duration::days(10)).to_rfc3339(),
                "duration_min":30,"interval_min":30,"max_participants":1,"image_url":".","config":{},"access_mode":"RESTRICTED"
            }).to_string())).unwrap()
    ).await.unwrap();

    let tok_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/revo/invitees", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    let tok_data = parse_body(tok_res).await;
    let token = tok_data["token"].as_str().unwrap();
    let invitee_id = tok_data["id"].as_str().unwrap();

    let rev_res = app.router.clone().oneshot(
        Request::builder().method("PUT").uri(format!("/api/v1/{}/invitees/{}", tid, invitee_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"status": "REVOKED"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(rev_res.status(), StatusCode::OK);

    let book_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/revo/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date":"2025-01-01","time":"10:00","name":"A","email":"a@a.com","token":token}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(book_res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_capacity_enforcement() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"Cap2","slug":"cap2"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug":"cap-test","title_en":"C","title_de":"C","desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
                "timezone": "UTC",
                "active_start":Utc::now().to_rfc3339(),"active_end":(Utc::now()+Duration::days(20)).to_rfc3339(),
                "duration_min":30,"interval_min":30,"max_participants":2,"image_url":".",
                "config":{"monday":[{"start":"09:00","end":"10:00"}]}, "access_mode":"OPEN"
            }).to_string())).unwrap()
    ).await.unwrap();

    let mut next_mon = Utc::now();
    while next_mon.format("%A").to_string() != "Monday" { next_mon += Duration::days(1); }
    next_mon += Duration::days(7);
    let date = next_mon.format("%Y-%m-%d").to_string();

    let b1 = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-test/book", tid)).header("Content-Type", "application/json").body(Body::from(json!({"date":date,"time":"09:00","name":"1","email":"1@1.com"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(b1.status(), StatusCode::OK);

    let b2 = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-test/book", tid)).header("Content-Type", "application/json").body(Body::from(json!({"date":date,"time":"09:00","name":"2","email":"2@2.com"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(b2.status(), StatusCode::OK);

    let b3 = app.router.clone().oneshot(Request::builder().method("POST").uri(format!("/api/v1/{}/events/cap-test/book", tid)).header("Content-Type", "application/json").body(Body::from(json!({"date":date,"time":"09:00","name":"3","email":"3@3.com"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(b3.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_date_validation() {
    let app = TestApp::new().await;
    let t_res = app.router.clone().oneshot(Request::builder().method("POST").uri("/api/v1/tenants").header("Content-Type", "application/json").body(Body::from(json!({"name":"V","slug":"val"}).to_string())).unwrap()).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap();
    let sec = t_data["admin_secret"].as_str().unwrap();
    let auth = app.login(tid, "admin", sec).await;

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "slug":"val-ev","title_en":"V","title_de":"V","desc_en":".","desc_de":".","location":".","payout":"0","host_name":"A",
                "timezone": "UTC",
                "active_start":Utc::now().to_rfc3339(),"active_end":(Utc::now()+Duration::days(10)).to_rfc3339(),
                "duration_min":30,"interval_min":30,"max_participants":1,"image_url":".","config":{"monday":[{"start":"09:00","end":"10:00"}]},"access_mode":"OPEN"
            }).to_string())).unwrap()
    ).await.unwrap();

    let res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/val-ev/book", tid))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"date":"2025/01/01","time":"09:00","name":"A","email":"a@a.com"}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
