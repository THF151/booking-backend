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
use tracing::debug;

pub struct MaybeAuthUser(pub Option<User>);

impl<S> FromRequestParts<S> for MaybeAuthUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = <Arc<AppState> as FromRef<S>>::from_ref(state);

        let cookies = parts.extensions.get::<Cookies>();
        if cookies.is_none() {
            return Ok(MaybeAuthUser(None));
        }
        let cookies = cookies.unwrap();

        let access_token = match cookies.get("access_token") {
            Some(cookie) => cookie.value().to_string(),
            None => return Ok(MaybeAuthUser(None)),
        };

        let decoding_key = DecodingKey::from_ed_pem(app_state.config.jwt_public_key.as_bytes());
        if decoding_key.is_err() {
            debug!("MaybeAuth: Invalid Public Key config");
            return Ok(MaybeAuthUser(None));
        }
        let decoding_key = decoding_key.unwrap();

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_audience(&["booking-frontend"]);

        let token_data = match decode::<Claims>(&access_token, &decoding_key, &validation) {
            Ok(data) => data,
            Err(_) => {
                // Invalid token (expired, bad signature) -> Treat as guest
                return Ok(MaybeAuthUser(None));
            }
        };

        let user = User {
            id: token_data.claims.sub,
            tenant_id: token_data.claims.tenant_id,
            username: "jwt_user".to_string(), // Placeholder
            role: token_data.claims.role,
            password_hash: "".to_string(),
            created_at: chrono::Utc::now(),
        };

        Ok(MaybeAuthUser(Some(user)))
    }
}