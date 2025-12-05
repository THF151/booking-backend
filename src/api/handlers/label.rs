use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::requests::CreateLabelRequest;
use crate::domain::models::booking::BookingLabel;
use crate::error::AppError;
use std::sync::Arc;
use tracing::info;

pub async fn list_labels(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let labels = state.label_repo.list(&tenant_id).await?;
    Ok(Json(labels))
}

pub async fn create_label(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<CreateLabelRequest>,
) -> Result<impl IntoResponse, AppError> {
    let payout = payload.payout.unwrap_or(0);
    let label = BookingLabel::new(tenant_id, payload.name, payload.color, payout);
    let created = state.label_repo.create(&label).await?;
    info!("Created label: {} with payout {}€", created.name, created.payout);
    Ok(Json(created))
}

pub async fn delete_label(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, label_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    state.label_repo.delete(&tenant_id, &label_id).await?;
    info!("Deleted label: {}", label_id);
    Ok(Json(serde_json::json!({"status": "deleted"})))
}