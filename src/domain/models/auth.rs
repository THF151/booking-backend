use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,

    #[serde(rename = "https://booking.com/claims/tenant_id")]
    pub tenant_id: String,

    #[serde(rename = "https://booking.com/claims/role")]
    pub role: String,

    #[serde(rename = "https://booking.com/claims/csrf")]
    pub csrf_token: String,
}

#[derive(Debug, FromRow)]
pub struct RefreshTokenRecord {
    pub token_hash: String,
    pub user_id: String,
    pub tenant_id: String,
    pub family_id: Uuid,
    pub generation_id: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub csrf_token: String,
    pub user: UserProfile,
}

#[derive(Serialize)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub role: String,
}