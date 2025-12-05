use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct EventSession {
    pub id: String,
    pub event_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub max_participants: i32,
    pub location: Option<String>,
    pub host_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl EventSession {
    pub fn new(event_id: String, start: DateTime<Utc>, end: DateTime<Utc>, max_p: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_id,
            start_time: start,
            end_time: end,
            max_participants: max_p,
            location: None,
            host_name: None,
            created_at: Utc::now(),
        }
    }
}