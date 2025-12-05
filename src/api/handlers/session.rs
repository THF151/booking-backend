use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::requests::{CreateSessionRequest, UpdateSessionRequest};
use crate::domain::models::session::EventSession;
use crate::error::AppError;
use std::sync::Arc;
use chrono::{NaiveDate, NaiveTime, Utc, TimeZone};
use chrono_tz::Tz;
use tracing::info;

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    if event.schedule_type != "MANUAL" {
        return Err(AppError::Validation("Sessions can only be created for MANUAL events".into()));
    }

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

    let date = NaiveDate::parse_from_str(&payload.date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date format".into()))?;

    let start_time = NaiveTime::parse_from_str(&payload.start_time, "%H:%M")
        .map_err(|_| AppError::Validation("Invalid start time".into()))?;

    let end_time = NaiveTime::parse_from_str(&payload.end_time, "%H:%M")
        .map_err(|_| AppError::Validation("Invalid end time".into()))?;

    if end_time <= start_time {
        return Err(AppError::Validation("End time must be after start time".into()));
    }

    let start_dt_tz = tz.from_local_datetime(&date.and_time(start_time))
        .single()
        .ok_or(AppError::Validation("Invalid local time".into()))?;

    let end_dt_tz = tz.from_local_datetime(&date.and_time(end_time))
        .single()
        .ok_or(AppError::Validation("Invalid local time".into()))?;

    let start_utc = start_dt_tz.with_timezone(&Utc);
    let end_utc = end_dt_tz.with_timezone(&Utc);

    let overlaps = state.session_repo.find_overlap(&event.id, start_utc, end_utc).await?;
    if !overlaps.is_empty() {
        return Err(AppError::Conflict("Session overlaps with an existing session".into()));
    }

    let session = EventSession::new(event.id, start_utc, end_utc, payload.max_participants);
    let created = state.session_repo.create(&session).await?;

    info!("Created manual session for event {}", slug);
    Ok(Json(created))
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let sessions = state.session_repo.list_by_event(&event.id).await?;
    Ok(Json(sessions))
}

pub async fn update_session(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug, session_id)): Path<(String, String, String)>,
    Json(payload): Json<UpdateSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let mut session = state.session_repo.find_by_id(&session_id).await?
        .ok_or(AppError::NotFound("Session not found".into()))?;

    if session.event_id != event.id {
        return Err(AppError::NotFound("Session not found for this event".into()));
    }

    if let Some(cap) = payload.max_participants {
        let bookings_count = state.booking_repo.count_overlap(&event.id, session.start_time, session.end_time).await?;
        if (bookings_count as i32) > cap {
            return Err(AppError::Conflict(format!("Cannot reduce capacity to {}. {} bookings already exist.", cap, bookings_count)));
        }
        session.max_participants = cap;
    }

    if let Some(loc) = payload.location {
        session.location = if loc.is_empty() { None } else { Some(loc) };
    }

    if let Some(host) = payload.host_name {
        session.host_name = if host.is_empty() { None } else { Some(host) };
    }

    let updated = state.session_repo.update(&session).await?;
    info!("Updated session {}", session_id);
    Ok(Json(updated))
}


pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug, session_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let session = state.session_repo.find_by_id(&session_id).await?
        .ok_or(AppError::NotFound("Session not found".into()))?;

    if session.event_id != event.id {
        return Err(AppError::NotFound("Session not found for this event".into()));
    }

    let bookings = state.booking_repo.count_overlap(&event.id, session.start_time, session.end_time).await?;
    if bookings > 0 {
        return Err(AppError::Conflict("Cannot delete session with existing bookings".into()));
    }

    state.session_repo.delete(&session_id).await?;
    info!("Deleted session {}", session_id);
    Ok(Json(serde_json::json!({"status": "deleted"})))
}