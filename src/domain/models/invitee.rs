use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use rand::{distributions::Alphanumeric, Rng};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Invitee {
    pub id: String,
    pub tenant_id: String,
    pub event_id: String,
    pub token: String,
    pub email: Option<String>,
    pub status: String, // ACTIVE, USED
    pub created_at: DateTime<Utc>,
}

impl Invitee {
    pub fn new(tenant_id: String, event_id: String, email: Option<String>) -> Self {
        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id,
            event_id,
            token,
            email,
            status: "ACTIVE".to_string(),
            created_at: Utc::now(),
        }
    }
}