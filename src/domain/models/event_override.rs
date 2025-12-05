use serde::{Deserialize, Serialize};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct EventOverride {
    pub id: String,
    pub event_id: String,
    pub date: NaiveDate,
    pub is_unavailable: bool,
    pub override_config_json: Option<String>,
    pub override_max_participants: Option<i32>,
    pub location: Option<String>,
    pub host_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl EventOverride {
    pub fn new(event_id: String, date: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_id,
            date,
            is_unavailable: false,
            override_config_json: None,
            override_max_participants: None,
            location: None,
            host_name: None,
            created_at: Utc::now(),
        }
    }
}