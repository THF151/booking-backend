use axum::{
    extract::{FromRequestParts, FromRef},
    http::{request::Parts, StatusCode},
};
use crate::state::AppState;
use crate::domain::models::auth::Claims;
use crate::domain::models::user::User;
use std::sync::Arc;
use tower_cookies::Cookies;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use tracing::Span;

pub struct AuthUser(pub User);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let cookies = parts.extensions.get::<Cookies>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        let access_token = cookies.get("access_token")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .value()
            .to_string();

        let app_state = <Arc<AppState> as FromRef<S>>::from_ref(state);

        let decoding_key = DecodingKey::from_ed_pem(app_state.config.jwt_public_key.as_bytes())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&["booking-frontend"]);

        let token_data = decode::<Claims>(&access_token, &decoding_key, &validation)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let method = &parts.method;
        if method != "GET" && method != "HEAD" && method != "OPTIONS" {
            let csrf_header_val = parts.headers.get("X-CSRF-Token")
                .ok_or(StatusCode::FORBIDDEN)?
                .to_str()
                .map_err(|_| StatusCode::FORBIDDEN)?;

            if csrf_header_val != token_data.claims.csrf_token {
                return Err(StatusCode::FORBIDDEN);
            }
        }

        let user = User {
            id: token_data.claims.sub.clone(),
            tenant_id: token_data.claims.tenant_id.clone(),
            username: "from_jwt".to_string(),
            role: token_data.claims.role,
            password_hash: "".to_string(),
            created_at: chrono::Utc::now(),
        };

        Span::current().record("tenant_id", &user.tenant_id);
        Span::current().record("user_id", &user.id);

        Ok(AuthUser(user))
    }
}