use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::{requests::{CreateInviteeRequest, UpdateInviteeRequest}};
use crate::domain::models::invitee::Invitee;
use crate::error::AppError;
use std::sync::Arc;
use tracing::info;

pub async fn create_invitee(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
    Json(payload): Json<CreateInviteeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let invitee = Invitee::new(tenant_id.clone(), event.id, payload.email);
    let created = state.invitee_repo.create(&invitee).await?;

    info!("Created invitee token for event {}", slug);

    Ok(Json(created))
}

pub async fn list_invitees(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let invitees = state.invitee_repo.list_by_event(&tenant_id, &event.id).await?;
    Ok(Json(invitees))
}

pub async fn update_invitee(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, invitee_id)): Path<(String, String)>,
    Json(payload): Json<UpdateInviteeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut invitee = state.invitee_repo.find_by_id(&tenant_id, &invitee_id).await?
        .ok_or(AppError::NotFound("Invitee not found".into()))?;

    invitee.status = payload.status;
    if let Some(email) = payload.email {
        invitee.email = Some(email);
    }

    let updated = state.invitee_repo.update(&invitee).await?;
    info!("Updated invitee: {}", invitee_id);
    Ok(Json(updated))
}

pub async fn delete_invitee(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, invitee_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    state.invitee_repo.delete(&tenant_id, &invitee_id).await?;
    info!("Deleted invitee: {}", invitee_id);
    Ok(Json(serde_json::json!({"status": "deleted"})))
}