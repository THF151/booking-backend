use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct User {
    pub id: String,
    pub tenant_id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn new(tenant_id: String, username: String, password_hash: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id,
            username,
            password_hash,
            role: "MEMBER".to_string(),
            created_at: Utc::now(),
        }
    }
}