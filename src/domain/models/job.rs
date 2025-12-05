use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobPayload {
    pub booking_id: String,
    pub tenant_id: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Job {
    pub id: String,
    pub job_type: String, // "CONFIRMATION" or "REMINDER"
    pub payload: Json<JobPayload>,
    pub execute_at: DateTime<Utc>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Job {
    pub fn new(job_type: &str, booking_id: String, tenant_id: String, execute_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            job_type: job_type.to_string(),
            payload: Json(JobPayload { booking_id, tenant_id }),
            execute_at,
            status: "PENDING".to_string(),
            error_message: None,
            created_at: Utc::now(),
        }
    }
}