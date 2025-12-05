mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_template_versioning_flow() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Version Corp", "slug": "ver-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tid = t_data["tenant_id"].as_str().unwrap().to_string();
    let sec = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tid, "admin", &sec).await;

    // 2. Create Template (V1)
    let create_payload = json!({
        "name": "Versioned Template",
        "subject_template": "Subject V1",
        "body_template": "Body V1",
        "template_type": "mjml"
    });

    let c_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/templates", tid))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(create_payload.to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(c_res.status(), StatusCode::OK);
    let template = parse_body(c_res).await;
    let tmpl_id = template["id"].as_str().unwrap();

    // 3. Update Template (V2)
    let update_payload = json!({
        "name": "Versioned Template",
        "subject_template": "Subject V2",
        "body_template": "Body V2",
        "template_type": "mjml"
    });

    let u_res = app.router.clone().oneshot(
        Request::builder().method("PUT").uri(format!("/api/v1/{}/templates/{}", tid, tmpl_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(update_payload.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(u_res.status(), StatusCode::OK);

    // 4. List Versions (Should be 2)
    let v_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/templates/{}/versions", tid, tmpl_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let versions = parse_body(v_res).await;
    let versions_arr = versions.as_array().unwrap();
    assert_eq!(versions_arr.len(), 2);

    let v1_id = versions_arr.iter().find(|v| v["body_template"] == "Body V1").unwrap()["id"].as_str().unwrap();

    // 5. Restore V1
    let r_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/templates/{}/versions/{}/restore", tid, tmpl_id, v1_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(r_res.status(), StatusCode::OK);

    // 6. Verify Template is back to V1
    let g_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/templates/{}", tid, tmpl_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let current = parse_body(g_res).await;
    assert_eq!(current["body_template"], "Body V1");
}