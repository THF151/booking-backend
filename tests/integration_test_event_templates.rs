mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;
use chrono::{Utc, Duration};

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_event_creation_generates_specific_templates() {
    let app = TestApp::new().await;

    // 1. Setup Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Template Corp", "slug": "tmpl-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &sec).await;

    // 2. Create Event
    let event_slug = "custom-event";
    let ev_payload = json!({
        "slug": event_slug,
        "title_en": "Custom", "title_de": "Benutzer", "desc_en": ".", "desc_de": ".",
        "location": "Web", "payout": "0", "host_name": "Host", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 30, "interval_min": 30, "max_participants": 1, "image_url": ".",
        "config": {}, "access_mode": "OPEN"
    });

    let ev_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(ev_res.status(), StatusCode::OK);
    let ev_data = parse_body(ev_res).await;
    let event_id = ev_data["id"].as_str().unwrap();

    // 3. List Templates (Should be auto-generated)
    let tmpl_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/templates", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let templates = parse_body(tmpl_res).await;
    let templates_arr = templates.as_array().unwrap();

    assert!(!templates_arr.is_empty(), "Should have generated templates");

    // 4. Verify content of generated template
    let confirmation_tmpl = templates_arr.iter()
        .find(|t| t["name"].as_str().unwrap().contains("Confirmation"))
        .expect("Confirmation template not found");

    // Check name includes slug (proving it's specific)
    let tmpl_name = confirmation_tmpl["name"].as_str().unwrap();
    assert!(tmpl_name.contains(event_slug), "Template name should contain event slug: {}", tmpl_name);

    // Check content is populated (not empty or error)
    let body = confirmation_tmpl["body_template"].as_str().unwrap();
    assert!(body.contains("<mjml>"), "Body should be MJML");
    assert!(!body.contains("Default template for"), "Body should not be error message");
    assert!(body.contains("Booking Confirmed"), "Body should contain confirmation text");

    // 5. Verify Rule Linking
    let rule_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/events/{}/rules", tenant_id, event_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let rules = parse_body(rule_res).await;
    let rules_arr = rules.as_array().unwrap();

    let conf_rule = rules_arr.iter()
        .find(|r| r["trigger_type"] == "ON_BOOKING")
        .expect("ON_BOOKING rule not found");

    assert_eq!(conf_rule["template_id"], confirmation_tmpl["id"], "Rule should link to the generated template");
}