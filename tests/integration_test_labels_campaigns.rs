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
async fn test_label_payout_integer_flow() {
    let app = TestApp::new().await;

    // 1. Create Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Label Corp", "slug": "lbl-test"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let admin_secret = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &admin_secret).await;

    // 2. Check default labels have integer payout
    let list_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/labels", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(list_res.status(), StatusCode::OK);
    let labels = parse_body(list_res).await;
    let labels_arr = labels.as_array().unwrap();

    // Expect 3 default labels
    assert_eq!(labels_arr.len(), 3);
    let show = labels_arr.iter().find(|l| l["name"] == "Show").unwrap();
    assert_eq!(show["payout"], 15); // Integer 15

    let noshow = labels_arr.iter().find(|l| l["name"] == "Noshow").unwrap();
    assert_eq!(noshow["payout"], 0); // Integer 0

    // 3. Create Custom Label with payout 50
    let create_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/labels", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "name": "VIP",
                "color": "#FFD700",
                "payout": 50
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(create_res.status(), StatusCode::OK);
    let created = parse_body(create_res).await;
    assert_eq!(created["payout"], 50);

    // 4. Assign Label to Booking (Integrity Check)
    let ev_payload = json!({
        "slug": "ev1", "title_en": "E", "title_de": "E", "desc_en": ".", "desc_de": ".",
        "location": ".", "payout": "0", "host_name": "H", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": { "monday": [{"start":"00:00", "end":"23:59"}], "tuesday": [{"start":"00:00", "end":"23:59"}], "wednesday": [{"start":"00:00", "end":"23:59"}], "thursday": [{"start":"00:00", "end":"23:59"}], "friday": [{"start":"00:00", "end":"23:59"}], "saturday": [{"start":"00:00", "end":"23:59"}], "sunday": [{"start":"00:00", "end":"23:59"}] },
        "access_mode": "OPEN"
    });

    app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();

    // Book tomorrow (should always be valid with above config)
    let date = (Utc::now() + Duration::days(1)).format("%Y-%m-%d").to_string();
    let booking_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/ev1/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date,
                "time": "10:00",
                "name": "Label Tester",
                "email": "lbl@test.com"
            }).to_string())).unwrap()
    ).await.unwrap();

    let booking = parse_body(booking_res).await;
    // Now this unwrap should succeed because booking creation succeeded
    let booking_id = booking["id"].as_str().expect("Booking ID missing - creation failed");

    // Assign Label
    let label_id = created["id"].as_str().unwrap();
    let update_res = app.router.clone().oneshot(
        Request::builder().method("PUT").uri(format!("/api/v1/{}/bookings/{}", tenant_id, booking_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"label_id": label_id}).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(update_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_campaign_scheduling_fix() {
    let app = TestApp::new().await;

    // 1. Tenant
    let t_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri("/api/v1/tenants")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"name": "Camp Fix Corp", "slug": "camp-fix"}).to_string())).unwrap()
    ).await.unwrap();
    let t_data = parse_body(t_res).await;
    let tenant_id = t_data["tenant_id"].as_str().unwrap().to_string();
    let admin_secret = t_data["admin_secret"].as_str().unwrap().to_string();
    let auth = app.login(&tenant_id, "admin", &admin_secret).await;

    // 2. Create Event and Template
    let ev_payload = json!({
        "slug": "camp-ev", "title_en": "C", "title_de": "C", "desc_en": ".", "desc_de": ".",
        "location": ".", "payout": "0", "host_name": "H", "timezone": "UTC",
        "active_start": Utc::now().to_rfc3339(), "active_end": (Utc::now()+Duration::days(30)).to_rfc3339(),
        "duration_min": 60, "interval_min": 60, "max_participants": 1, "image_url": ".",
        "config": { "monday": [{"start":"00:00", "end":"23:59"}], "tuesday": [{"start":"00:00", "end":"23:59"}], "wednesday": [{"start":"00:00", "end":"23:59"}], "thursday": [{"start":"00:00", "end":"23:59"}], "friday": [{"start":"00:00", "end":"23:59"}], "saturday": [{"start":"00:00", "end":"23:59"}], "sunday": [{"start":"00:00", "end":"23:59"}] },
        "access_mode": "OPEN"
    });

    let ev_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(ev_payload.to_string())).unwrap()
    ).await.unwrap();
    let event = parse_body(ev_res).await;
    let event_id = event["id"].as_str().unwrap();

    let tmpl_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/templates", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "name": "Campaign Tmpl",
                "subject_template": "Hi",
                "body_template": "Body",
                "template_type": "html",
                "event_id": event_id
            }).to_string())).unwrap()
    ).await.unwrap();
    let template = parse_body(tmpl_res).await;
    let template_id = template["id"].as_str().unwrap();

    // 3. Create Bookings (Recipients)
    let date = (Utc::now()+Duration::days(1)).format("%Y-%m-%d").to_string();
    let b1_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/events/camp-ev/book", tenant_id))
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "date": date,
                "time": "10:00", "name": "R1", "email": "r1@test.com"
            }).to_string())).unwrap()
    ).await.unwrap();
    let b1 = parse_body(b1_res).await;
    let b1_id = b1["id"].as_str().expect("Booking failed");

    // 4. Schedule Campaign
    let send_res = app.router.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/v1/{}/campaigns/send", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({
                "template_id": template_id,
                "target_type": "BOOKING",
                "recipients": [b1_id]
            }).to_string())).unwrap()
    ).await.unwrap();

    assert_eq!(send_res.status(), StatusCode::OK);
    let send_data = parse_body(send_res).await;
    assert_eq!(send_data["status"], "queued");
    assert_eq!(send_data["count"], 1);

    // 5. Check Job Queue
    let jobs_res = app.router.clone().oneshot(
        Request::builder().method("GET").uri(format!("/api/v1/{}/jobs", tenant_id))
            .header(header::COOKIE, format!("access_token={}", auth.access_token))
            .header("X-CSRF-Token", &auth.csrf_token)
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    let jobs = parse_body(jobs_res).await;
    let jobs_arr = jobs.as_array().unwrap();

    // Should have at least 2 jobs: 1 Confirmation (auto), 1 Campaign
    assert!(jobs_arr.len() >= 2);
    let campaign_job = jobs_arr.iter().find(|j| j["job_type"].as_str().unwrap().starts_with("CAMPAIGN"));
    assert!(campaign_job.is_some());

    // Verify job type structure
    let job_type = campaign_job.unwrap()["job_type"].as_str().unwrap();
    assert!(job_type.contains(template_id));
    assert!(job_type.contains("BOOKING"));
}