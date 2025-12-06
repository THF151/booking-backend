use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use rand::{distributions::Alphanumeric, Rng};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Booking {
    pub id: String,
    pub tenant_id: String,
    pub event_id: String,
    pub invitee_id: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub customer_name: String,
    pub customer_email: String,
    pub customer_note: Option<String>,
    pub location: Option<String>,
    pub label_id: Option<String>,
    pub status: String,
    pub management_token: String,
    pub token: Option<String>,
    pub payout: Option<i32>,
    pub created_at: DateTime<Utc>,
}

pub struct NewBookingParams {
    pub tenant_id: String,
    pub event_id: String,
    pub start: DateTime<Utc>,
    pub duration_min: i32,
    pub name: String,
    pub email: String,
    pub note: Option<String>,
    pub invitee_id: Option<String>,
    pub location: Option<String>,
}

impl Booking {
    pub fn new(params: NewBookingParams) -> Self {
        let end_time = params.start + chrono::Duration::minutes(params.duration_min as i64);

        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(48)
            .map(char::from)
            .collect();

        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id: params.tenant_id,
            event_id: params.event_id,
            invitee_id: params.invitee_id,
            start_time: params.start,
            end_time,
            customer_name: params.name,
            customer_email: params.email,
            customer_note: params.note,
            location: params.location,
            label_id: None,
            status: "CONFIRMED".to_string(),
            management_token: token,
            token: None,
            payout: None,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct BookingLabel {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub color: String,
    pub payout: i32,
    pub created_at: DateTime<Utc>,
}

impl BookingLabel {
    pub fn new(tenant_id: String, name: String, color: String, payout: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id,
            name,
            color,
            payout,
            created_at: Utc::now(),
        }
    }
}