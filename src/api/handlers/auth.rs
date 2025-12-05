use axum::{extract::State, response::IntoResponse, Json, http::StatusCode};
use crate::state::AppState;
use crate::error::AppError;
use crate::domain::models::auth::{AuthResponse, UserProfile};
use std::sync::Arc;
use tower_cookies::{Cookies, Cookie};
use tower_cookies::cookie::SameSite;
use time::Duration;
use argon2::{PasswordHash, Argon2, PasswordVerifier};
use tracing::info;

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub tenant_id: String,
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = state.user_repo.find_by_username(&payload.tenant_id, &payload.username).await?
        .ok_or(AppError::Unauthorized)?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|_| AppError::Internal)?;

    Argon2::default().verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)?;

    let (access_jwt, refresh_token, csrf_token) = state.auth_service.login(&user).await?;

    set_cookies(&cookies, &access_jwt, &refresh_token);

    info!("User logged in: {}", user.id);

    Ok(Json(AuthResponse {
        csrf_token,
        user: UserProfile {
            id: user.id,
            username: user.username,
            role: user.role,
        }
    }))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
) -> Result<impl IntoResponse, AppError> {
    let refresh_cookie = cookies.get("refresh_token").ok_or(AppError::Unauthorized)?;
    let raw_token = refresh_cookie.value();

    let token_hash = state.auth_service.hash_token(raw_token);
    let record = state.auth_repo.find_refresh_token(&token_hash).await?
        .ok_or(AppError::Unauthorized)?;

    let user = state.user_repo.find_by_id(&record.tenant_id, &record.user_id).await?
        .ok_or(AppError::Unauthorized)?;

    let (new_access, new_refresh, new_csrf) = state.auth_service.refresh(raw_token, &user).await?;

    set_cookies(&cookies, &new_access, &new_refresh);

    info!("Token refreshed for user: {}", user.id);

    Ok(Json(AuthResponse {
        csrf_token: new_csrf,
        user: UserProfile {
            id: user.id,
            username: user.username,
            role: user.role,
        }
    }))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
) -> Result<impl IntoResponse, AppError> {
    if let Some(cookie) = cookies.get("refresh_token") {
        let _ = state.auth_service.logout(cookie.value()).await;
    }

    cookies.remove(Cookie::build(("access_token", "")).path("/").into());
    cookies.remove(Cookie::build(("refresh_token", "")).path("/").into());

    info!("User logged out");

    Ok(StatusCode::OK)
}

fn set_cookies(cookies: &Cookies, access: &str, refresh: &str) {
    let mut access_c = Cookie::new("access_token", access.to_string());
    access_c.set_http_only(true);
    access_c.set_secure(true);
    access_c.set_same_site(SameSite::Strict);
    access_c.set_path("/");
    access_c.set_max_age(Duration::minutes(15));
    cookies.add(access_c);

    let mut refresh_c = Cookie::new("refresh_token", refresh.to_string());
    refresh_c.set_http_only(true);
    refresh_c.set_secure(true);
    refresh_c.set_same_site(SameSite::Strict);
    refresh_c.set_path("/");
    refresh_c.set_max_age(Duration::days(7));
    cookies.add(refresh_c);
}