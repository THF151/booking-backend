use axum::{
    body::Body,
    extract::Request,
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use crate::state::AppState;
use crate::api::handlers::{health, tenant, event, booking, invitee, member, event_override, auth, label, session, booking_management, communication, ai};
use tower_http::{
    trace::TraceLayer,
    classify::ServerErrorsFailureClass,
};
use tower_cookies::CookieManagerLayer;
use tracing::{info_span, Span, error, info};
use uuid::Uuid;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::health_check))

        // Auth
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))

        // Tenant Public
        .route("/api/v1/tenants/by-slug/{slug}", get(tenant::get_tenant_by_slug))

        // Tenant Admin
        .route("/api/v1/tenants", post(tenant::create_tenant).put(tenant::update_tenant).get(tenant::get_current_tenant))
        .route("/api/v1/{tenant_id}/members", post(member::create_member).get(member::list_members))
        .route("/api/v1/{tenant_id}/members/{user_id}", delete(member::delete_member))

        // AI
        .route("/api/v1/{tenant_id}/ai/generate", post(ai::generate_content))

        // Config
        .route("/api/v1/{tenant_id}/labels", get(label::list_labels).post(label::create_label))
        .route("/api/v1/{tenant_id}/labels/{label_id}", delete(label::delete_label).put(label::update_label))

        // Communication - Templates
        .route("/api/v1/{tenant_id}/templates", get(communication::list_templates).post(communication::create_template))
        .route("/api/v1/{tenant_id}/templates/{template_id}", get(communication::get_template).put(communication::update_template).delete(communication::delete_template))

        // Communication - Versioning
        .route("/api/v1/{tenant_id}/templates/{template_id}/versions", get(communication::list_versions))
        .route("/api/v1/{tenant_id}/templates/{template_id}/versions/{version_id}/restore", post(communication::restore_version))

        // Communication - Rules
        .route("/api/v1/{tenant_id}/events/{event_id}/rules", get(communication::list_event_rules).post(communication::create_event_rule))
        .route("/api/v1/{tenant_id}/rules/{rule_id}", delete(communication::delete_rule))

        // Communication - Meta & Test
        .route("/api/v1/communication/placeholders", get(communication::get_placeholders))
        .route("/api/v1/{tenant_id}/communication/test-send", post(communication::send_test_email))
        .route("/api/v1/{tenant_id}/communication/logs", get(communication::get_logs))

        // Campaigns & Jobs
        .route("/api/v1/{tenant_id}/campaigns/preview", post(communication::preview_campaign))
        .route("/api/v1/{tenant_id}/campaigns/send", post(communication::send_campaign))
        .route("/api/v1/{tenant_id}/jobs", get(communication::list_jobs))

        // Events
        .route("/api/v1/{tenant_id}/events", post(event::create_event).get(event::list_events))
        .route("/api/v1/{tenant_id}/events/{slug}", get(event::get_event).put(event::update_event).delete(event::delete_event))
        .route("/api/v1/{tenant_id}/events/{slug}/invitees", post(invitee::create_invitee).get(invitee::list_invitees))
        .route("/api/v1/{tenant_id}/invitees/{invitee_id}", put(invitee::update_invitee).delete(invitee::delete_invitee))

        // Overrides & Sessions
        .route("/api/v1/{tenant_id}/events/{slug}/overrides", get(event_override::list_overrides).post(event_override::upsert_override))
        .route("/api/v1/{tenant_id}/events/{slug}/overrides/{date}", delete(event_override::delete_override))
        .route("/api/v1/{tenant_id}/events/{slug}/sessions", get(session::list_sessions).post(session::create_session))
        .route("/api/v1/{tenant_id}/events/{slug}/sessions/{session_id}", put(session::update_session).delete(session::delete_session))

        // Public Booking Flow
        .route("/api/v1/{tenant_id}/events/{slug}/dates", get(event::get_available_dates))
        .route("/api/v1/{tenant_id}/events/{slug}/slots", get(event::get_slots))
        .route("/api/v1/{tenant_id}/events/{slug}/book", post(booking::create_booking))

        // Customer Booking Management
        .route("/api/v1/bookings/manage/{token}", get(booking_management::get_booking_by_token))
        .route("/api/v1/bookings/manage/{token}/cancel", post(booking_management::cancel_booking))
        .route("/api/v1/bookings/manage/{token}/reschedule", post(booking_management::reschedule_booking))

        // Admin Booking Management
        .route("/api/v1/{tenant_id}/events/{slug}/bookings", get(booking::list_bookings))
        .route("/api/v1/{tenant_id}/bookings/{booking_id}", get(booking::get_booking).put(booking::update_booking).delete(booking::delete_booking))
        .route("/api/v1/{tenant_id}/bookings", get(booking::list_all_bookings))

        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    let request_id = Uuid::new_v4().to_string();
                    info_span!(
                        "http_request",
                        request_id = %request_id,
                        method = ?request.method(),
                        uri = ?request.uri(),
                        version = ?request.version(),
                        tenant_id = tracing::field::Empty,
                        user_id = tracing::field::Empty,
                    )
                })
                .on_request(|request: &Request<Body>, _span: &Span| {
                    info!("started processing request: {} {}", request.method(), request.uri().path());
                })
                .on_response(|response: &axum::http::Response<Body>, latency: Duration, _span: &Span| {
                    info!(
                        status = response.status().as_u16(),
                        latency_ms = latency.as_millis(),
                        "finished processing request"
                    );
                })
                .on_failure(|error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                    error!("request failed: {:?}", error);
                })
        )
        .layer(CookieManagerLayer::new())
        .with_state(state)
}