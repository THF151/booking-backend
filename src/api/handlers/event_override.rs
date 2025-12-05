use axum::{extract::{State, Path, Query}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::requests::EventOverrideRequest;
use crate::domain::models::event_override::EventOverride;
use crate::error::AppError;
use std::sync::Arc;
use chrono::NaiveDate;
use std::collections::HashMap;
use tracing::info;

pub async fn upsert_override(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
    Json(payload): Json<EventOverrideRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    if event.schedule_type == "MANUAL" {
        return Err(AppError::Validation("Overrides not applicable for MANUAL events".into()));
    }

    let override_config_json = if let Some(cfg) = payload.config {
        Some(serde_json::to_string(&cfg).map_err(|_| AppError::Validation("Invalid config".into()))?)
    } else {
        None
    };

    let entity = EventOverride {
        id: uuid::Uuid::new_v4().to_string(),
        event_id: event.id,
        date: payload.date,
        is_unavailable: payload.is_unavailable,
        override_config_json,
        override_max_participants: payload.override_max_participants,
        location: payload.location,
        host_name: payload.host_name,
        created_at: chrono::Utc::now(),
    };

    let saved = state.event_override_repo.upsert(&entity).await?;
    info!("Upserted override for event {} on {}", slug, payload.date);
    Ok(Json(saved))
}

pub async fn delete_override(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug, date_str)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    state.event_override_repo.delete(&event.id, date).await?;
    info!("Deleted override for event {} on {}", slug, date_str);
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn list_overrides(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let start_str = params.get("start").ok_or(AppError::Validation("start required".into()))?;
    let end_str = params.get("end").ok_or(AppError::Validation("end required".into()))?;

    let start = NaiveDate::parse_from_str(start_str, "%Y-%m-%d").map_err(|_| AppError::Validation("Invalid start".into()))?;
    let end = NaiveDate::parse_from_str(end_str, "%Y-%m-%d").map_err(|_| AppError::Validation("Invalid end".into()))?;

    let overrides = state.event_override_repo.list_by_range(&event.id, start, end).await?;
    Ok(Json(overrides))
}
