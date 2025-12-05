use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub logo_url: Option<String>,
    #[serde(skip_serializing)]
    pub ai_api_key: Option<String>,
    pub ai_provider: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Tenant {
    pub fn new(name: String, slug: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            slug,
            logo_url: None,
            ai_api_key: None,
            ai_provider: Some("gemini".to_string()),
            created_at: Utc::now(),
        }
    }
}