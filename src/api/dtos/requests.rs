use crate::domain::models::event::WeekdayConfig;
use chrono::{DateTime, Utc, NaiveDate};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    pub logo_url: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateTenantRequest {
    pub name: Option<String>,
    pub logo_url: Option<String>,
    pub ai_api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateMemberRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateEventRequest {
    pub slug: String,
    pub title_en: String,
    pub title_de: String,
    pub desc_en: String,
    pub desc_de: String,
    pub location: String,
    pub payout: String,
    pub host_name: String,
    pub timezone: String,
    pub min_notice_general: Option<i32>,
    pub min_notice_first: Option<i32>,
    pub active_start: DateTime<Utc>,
    pub active_end: DateTime<Utc>,
    pub duration_min: i32,
    pub interval_min: i32,
    pub max_participants: i32,
    pub image_url: String,
    pub config: WeekdayConfig,
    pub access_mode: String,
    pub schedule_type: Option<String>,
    pub allow_customer_cancel: Option<bool>,
    pub allow_customer_reschedule: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateEventRequest {
    pub slug: Option<String>,
    pub title_en: Option<String>,
    pub title_de: Option<String>,
    pub desc_en: Option<String>,
    pub desc_de: Option<String>,
    pub location: Option<String>,
    pub payout: Option<String>,
    pub host_name: Option<String>,
    pub timezone: Option<String>,
    pub min_notice_general: Option<i32>,
    pub min_notice_first: Option<i32>,
    pub active_start: Option<DateTime<Utc>>,
    pub active_end: Option<DateTime<Utc>>,
    pub duration_min: Option<i32>,
    pub interval_min: Option<i32>,
    pub max_participants: Option<i32>,
    pub image_url: Option<String>,
    pub config: Option<WeekdayConfig>,
    pub access_mode: Option<String>,
    pub schedule_type: Option<String>,
    pub allow_customer_cancel: Option<bool>,
    pub allow_customer_reschedule: Option<bool>,
}

#[derive(Deserialize)]
pub struct CreateInviteeRequest {
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateInviteeRequest {
    pub status: String,
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateBookingRequest {
    pub date: String,
    pub time: String,
    pub name: String,
    pub email: String,
    pub notes: Option<String>,
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct RescheduleBookingRequest {
    pub date: String,
    pub time: String,
}

#[derive(Deserialize)]
pub struct UpdateBookingRequest {
    pub date: Option<String>,
    pub time: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub label_id: Option<String>,
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct EventOverrideRequest {
    pub date: NaiveDate,
    pub is_unavailable: bool,
    pub config: Option<WeekdayConfig>,
    pub override_max_participants: Option<i32>,
    pub location: Option<String>,
    pub host_name: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: String,
    pub payout: Option<i32>,
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub max_participants: i32,
    pub location: Option<String>,
    pub host_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateSessionRequest {
    pub max_participants: Option<i32>,
    pub location: Option<String>,
    pub host_name: Option<String>,
}