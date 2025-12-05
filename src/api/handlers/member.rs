use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::requests::CreateMemberRequest;
use crate::domain::models::user::User;
use std::sync::Arc;
use crate::error::AppError;
use argon2::{password_hash::{SaltString, PasswordHasher}, Argon2};
use rand::rngs::OsRng;
use tracing::{info, error};

pub async fn create_member(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _admin: AuthUser,
    Json(payload): Json<CreateMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    if state.user_repo.find_by_username(&tenant_id, &payload.username).await?.is_some() {
        return Err(AppError::Conflict("Username already exists".into()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();

    let user = User::new(tenant_id, payload.username, password_hash);
    let created = state.user_repo.create(&user).await?;

    info!("Created member user: {}", created.id);

    Ok(Json(serde_json::json!({
        "id": created.id,
        "username": created.username,
        "role": created.role,
        "created_at": created.created_at
    })))
}

pub async fn list_members(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _admin: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let members = state.user_repo.list_by_tenant(&tenant_id).await?;
    let safe_members: Vec<_> = members.into_iter().map(|u| serde_json::json!({
        "id": u.id,
        "username": u.username,
        "role": u.role,
        "created_at": u.created_at
    })).collect();

    Ok(Json(safe_members))
}

pub async fn delete_member(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    admin: AuthUser,
    Path((_, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if admin.0.id == user_id {
        return Err(AppError::Conflict("Cannot delete yourself".into()));
    }

    let target = state.user_repo.find_by_id(&tenant_id, &user_id).await?
        .ok_or(AppError::NotFound("User not found".into()))?;

    match state.user_repo.delete(&tenant_id, &target.id).await {
        Ok(_) => {
            info!("Deleted user {}", user_id);
            Ok(Json(serde_json::json!({"status": "deleted"})))
        },
        Err(e) => {
            error!("Failed to delete user {}: {:?}", user_id, e);
            Err(e)
        }
    }
}