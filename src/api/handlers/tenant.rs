use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::dtos::{
    requests::{CreateTenantRequest, UpdateTenantRequest},
    responses::TenantCreatedResponse
};
use crate::api::extractors::auth::AuthUser;
use crate::domain::models::{tenant::Tenant, user::User, booking::BookingLabel};
use std::sync::Arc;
use crate::error::AppError;
use rand::{distributions::Alphanumeric, Rng};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use tracing::info;

pub async fn create_tenant(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut tenant = Tenant::new(payload.name, payload.slug);
    if let Some(logo) = payload.logo_url {
        tenant.logo_url = Some(logo);
    }

    let created_tenant = state.tenant_repo.create(&tenant).await?;

    info!("Tenant created: {}", created_tenant.id);

    let admin_password: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();

    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::default()
        .hash_password(admin_password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();

    let admin_user = User::new(created_tenant.id.clone(), "admin".to_string(), password_hash);
    state.user_repo.create(&admin_user).await?;

    let defaults = vec![
        ("Show", "#2e7d32", 15),
        ("Noshow", "#d32f2f", 0),
        ("Abgesagt", "#9e9e9e", 0),
    ];

    for (name, color, payout) in defaults {
        let label = BookingLabel::new(created_tenant.id.clone(), name.to_string(), color.to_string(), payout);
        let _ = state.label_repo.create(&label).await;
    }

    Ok(Json(TenantCreatedResponse {
        tenant_id: created_tenant.id,
        admin_username: "admin".to_string(),
        admin_secret: admin_password,
    }))
}

pub async fn get_tenant_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let tenant = state.tenant_repo.find_by_slug(&slug).await?
        .ok_or(AppError::NotFound("Tenant not found".into()))?;

    Ok(Json(tenant))
}

pub async fn update_tenant(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(payload): Json<UpdateTenantRequest>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = user.0.tenant_id;
    let mut tenant = state.tenant_repo.find_by_id(&tenant_id).await?
        .ok_or(AppError::NotFound("Tenant not found".into()))?;

    if let Some(name) = payload.name {
        tenant.name = name;
    }
    if let Some(logo) = payload.logo_url {
        tenant.logo_url = Some(logo);
    }
    if let Some(key) = payload.ai_api_key {
        tenant.ai_api_key = Some(key);
    }

    let updated = state.tenant_repo.update(&tenant).await?;
    info!("Tenant updated: {}", tenant_id);
    Ok(Json(updated))
}

pub async fn get_current_tenant(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = user.0.tenant_id;
    let tenant = state.tenant_repo.find_by_id(&tenant_id).await?
        .ok_or(AppError::NotFound("Tenant not found".into()))?;
    Ok(Json(tenant))
}