mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use common::TestApp;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn parse_body(response: axum::response::Response) -> Value {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    if bytes.is_empty() {
        panic!("Response body is empty. Status: {}", status);
    }
    match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => panic!("Failed to parse JSON: {:?}. Status: {}. Body: {:?}", e, status, String::from_utf8_lossy(&bytes))
    }
}

#[tokio::test]
async fn test_tenant_management_and_team_members() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Team Corp", "slug": "team-corp"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let admin_secret = t_data["admin_secret"].as_str().unwrap().to_string();

    // Login as Admin
    let auth = app.login(&tenant_id, "admin", &admin_secret).await;

    // 2. Update Tenant Settings (Logo)
    let update_res = app.router.clone().oneshot(
        Request::builder().method("PUT").uri("/api/v1/tenants")
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "name": "Team Corp Updated",
                "logo_url": "http://logo.com/img.png"
            }).to_string())).unwrap()
    ).await.unwrap();

    if update_res.status() != StatusCode::OK {
        let status = update_res.status();
        let body = parse_body(update_res).await;
        panic!("Update failed: status {}, body: {:?}", status, body);
    }
    let updated_tenant = parse_body(update_res).await;
    assert_eq!(updated_tenant["name"], "Team Corp Updated");
    assert_eq!(updated_tenant["logo_url"], "http://logo.com/img.png");

    // 3. Create Team Member
    let member_payload = json!({
        "username": "support",
        "password": "securepassword123"
    });

    let create_mem_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/members", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(member_payload.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(create_mem_res.status(), StatusCode::OK);
    let member_data = parse_body(create_mem_res).await;
    let member_id = member_data["id"].as_str().unwrap().to_string();

    // 4. List Members
    let list_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/members", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let members = parse_body(list_res).await;
    let members_arr = members.as_array().unwrap();
    assert_eq!(members_arr.len(), 2); // Admin + Support

    // 5. Login as new Member
    let member_auth = app.login(&tenant_id, "support", "securepassword123").await;
    assert!(!member_auth.access_token.is_empty());

    // 6. Get Tenant by Slug (Public)
    let slug_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri("/api/v1/tenants/by-slug/team-corp")
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    if slug_res.status() != StatusCode::OK {
        let status = slug_res.status();
        let body = parse_body(slug_res).await;
        panic!("Get by slug failed: status {}, body: {:?}", status, body);
    }

    let slug_data = parse_body(slug_res).await;
    assert_eq!(slug_data["id"], tenant_id);
    assert_eq!(slug_data["logo_url"], "http://logo.com/img.png");

    // 7. Delete Member
    let delete_res = app.router.clone().oneshot(
        Request::builder().method("DELETE").uri(format!("/api/v1/{}/members/{}", tenant_id, member_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    let status = delete_res.status();
    if status != StatusCode::OK {
        let body = parse_body(delete_res).await;
        panic!("Delete failed with status: {}. Body: {:?}", status, body);
    }
    // Consume body to satisfy test if needed, or just ignore
    let _ = parse_body(delete_res).await;

    // Verify Gone
    let list_res_2 = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/members", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let members_2 = parse_body(list_res_2).await;
    assert_eq!(members_2.as_array().unwrap().len(), 1);
}