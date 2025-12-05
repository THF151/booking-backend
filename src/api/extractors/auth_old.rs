use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{Response, IntoResponse},
};
use crate::state::AppState;
use crate::domain::models::user::User;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use super::tenant::TenantId;

pub struct AuthUser(pub User);

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let TenantId(tenant_id) = TenantId::from_request_parts(parts, state)
            .await
            .map_err(|code| code.into_response())?;

        let auth_header = parts.headers.get("Authorization")
            .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?
            .to_str()
            .map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

        if !auth_header.starts_with("Basic ") {
            return Err(StatusCode::UNAUTHORIZED.into_response());
        }

        let credentials = auth_header.trim_start_matches("Basic ");
        let decoded = general_purpose::STANDARD
            .decode(credentials)
            .map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

        let creds_str = String::from_utf8(decoded)
            .map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

        let mut split = creds_str.splitn(2, ':');
        let username = split.next().ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;
        let password = split.next().ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

        let user = state.user_repo.find_by_username(&tenant_id, username).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
            .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

        if Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok() {
            Ok(AuthUser(user))
        } else {
            Err(StatusCode::UNAUTHORIZED.into_response())
        }
    }
}