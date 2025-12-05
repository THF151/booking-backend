use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimeWindow {
    pub start: String,
    pub end: String,
    pub max_participants: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct WeekdayConfig {
    pub monday: Option<Vec<TimeWindow>>,
    pub tuesday: Option<Vec<TimeWindow>>,
    pub wednesday: Option<Vec<TimeWindow>>,
    pub thursday: Option<Vec<TimeWindow>>,
    pub friday: Option<Vec<TimeWindow>>,
    pub saturday: Option<Vec<TimeWindow>>,
    pub sunday: Option<Vec<TimeWindow>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Event {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub title_en: String,
    pub title_de: String,
    pub desc_en: String,
    pub desc_de: String,
    pub location: String,
    pub payout: String,
    pub host_name: String,
    pub timezone: String,
    pub min_notice_general: i32,
    pub min_notice_first: i32,
    pub active_start: DateTime<Utc>,
    pub active_end: DateTime<Utc>,
    pub duration_min: i32,
    pub interval_min: i32,
    pub max_participants: i32,
    pub image_url: String,
    pub config_json: String,
    pub access_mode: String,
    pub schedule_type: String,
    pub allow_customer_cancel: bool,
    pub allow_customer_reschedule: bool,
    pub created_at: DateTime<Utc>,
}
