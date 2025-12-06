use crate::domain::models::{
    tenant::Tenant, user::User, event::Event, booking::{Booking, BookingLabel},
    invitee::Invitee, event_override::EventOverride, job::Job, session::EventSession,
    auth::RefreshTokenRecord, communication::{EmailTemplate, EmailTemplateVersion, NotificationRule, MailLog}
};
use crate::error::AppError;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn create(&self, tenant: &Tenant) -> Result<Tenant, AppError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Tenant>, AppError>;
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Tenant>, AppError>;
    async fn update(&self, tenant: &Tenant) -> Result<Tenant, AppError>;
}

#[async_trait]
pub trait LlmService: Send + Sync {
    async fn generate(
        &self,
        api_key: &str,
        prompt: &str,
        system_instruction: &str
    ) -> Result<String, AppError>;
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<User, AppError>;
    async fn find_by_username(&self, tenant_id: &str, username: &str) -> Result<Option<User>, AppError>;
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<User>, AppError>;
    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<User>, AppError>;
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn create_refresh_token(&self, record: &RefreshTokenRecord) -> Result<(), AppError>;
    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<RefreshTokenRecord>, AppError>;
    async fn delete_refresh_token(&self, token_hash: &str) -> Result<(), AppError>;
    async fn delete_refresh_family(&self, family_id: Uuid) -> Result<(), AppError>;
}

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn create(&self, event: &Event) -> Result<Event, AppError>;
    async fn find_by_slug(&self, tenant_id: &str, slug: &str) -> Result<Option<Event>, AppError>;
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Event>, AppError>;
    async fn list(&self, tenant_id: &str) -> Result<Vec<Event>, AppError>;
    async fn update(&self, event: &Event) -> Result<Event, AppError>;
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait EventOverrideRepository: Send + Sync {
    async fn upsert(&self, override_entity: &EventOverride) -> Result<EventOverride, AppError>;
    async fn find_by_date(&self, event_id: &str, date: NaiveDate) -> Result<Option<EventOverride>, AppError>;
    async fn list_by_range(&self, event_id: &str, start: NaiveDate, end: NaiveDate) -> Result<Vec<EventOverride>, AppError>;
    async fn delete(&self, event_id: &str, date: NaiveDate) -> Result<(), AppError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, session: &EventSession) -> Result<EventSession, AppError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<EventSession>, AppError>;
    async fn list_by_event(&self, event_id: &str) -> Result<Vec<EventSession>, AppError>;
    async fn list_by_range(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<EventSession>, AppError>;
    async fn update(&self, session: &EventSession) -> Result<EventSession, AppError>;
    async fn delete(&self, id: &str) -> Result<(), AppError>;
    async fn find_overlap(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<EventSession>, AppError>;
}

#[async_trait]
pub trait BookingRepository: Send + Sync {
    async fn create(&self, booking: &Booking) -> Result<Booking, AppError>;
    async fn create_with_token(&self, booking: &Booking, token: Option<String>, jobs: Vec<Job>) -> Result<Booking, AppError>;
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Booking>, AppError>;
    async fn find_by_token(&self, token: &str) -> Result<Option<Booking>, AppError>;
    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Booking>, AppError>;
    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<Booking>, AppError>;
    async fn list_by_range(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Booking>, AppError>;
    async fn update(&self, booking: &Booking) -> Result<Booking, AppError>;
    async fn cancel(&self, booking: &Booking) -> Result<Booking, AppError>;
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError>;
    async fn count_overlap(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64, AppError>;
    async fn find_future_active_bookings(&self, event_id: &str) -> Result<Vec<Booking>, AppError>;
}

#[async_trait]
pub trait BookingLabelRepository: Send + Sync {
    async fn create(&self, label: &BookingLabel) -> Result<BookingLabel, AppError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<BookingLabel>, AppError>;
    async fn update(&self, label: &BookingLabel) -> Result<BookingLabel, AppError>;
    async fn list(&self, tenant_id: &str) -> Result<Vec<BookingLabel>, AppError>;
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create(&self, job: &Job) -> Result<Job, AppError>;
    async fn find_pending(&self, limit: i32) -> Result<Vec<Job>, AppError>;
    async fn list_jobs(&self, tenant_id: &str) -> Result<Vec<Job>, AppError>;
    async fn update_status(&self, id: &str, status: &str, error_message: Option<String>) -> Result<(), AppError>;
    async fn cancel_jobs_for_booking(&self, booking_id: &str) -> Result<(), AppError>;
    async fn delete_jobs_by_type_and_event(&self, event_id: &str, job_type: &str) -> Result<(), AppError>;
    async fn find_future_bookings_for_event(&self, event_id: &str) -> Result<Vec<Booking>, AppError>;
}

#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send(&self, recipient: &str, subject: &str, html_body: &str, attachment_name: Option<&str>, attachment_data: Option<&[u8]>) -> Result<(), AppError>;
}

#[async_trait]
pub trait InviteeRepository: Send + Sync {
    async fn create(&self, invitee: &Invitee) -> Result<Invitee, AppError>;
    async fn find_by_token(&self, token: &str) -> Result<Option<Invitee>, AppError>;
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Invitee>, AppError>;
    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Invitee>, AppError>;
    async fn update(&self, invitee: &Invitee) -> Result<Invitee, AppError>;
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait CommunicationRepository: Send + Sync {
    async fn create_template(&self, template: &EmailTemplate) -> Result<EmailTemplate, AppError>;
    async fn get_template(&self, id: &str) -> Result<Option<EmailTemplate>, AppError>;
    async fn list_templates(&self, tenant_id: &str, event_id: Option<&str>) -> Result<Vec<EmailTemplate>, AppError>;
    async fn update_template(&self, template: &EmailTemplate) -> Result<EmailTemplate, AppError>;
    async fn delete_template(&self, id: &str) -> Result<(), AppError>;

    async fn create_template_version(&self, version: &EmailTemplateVersion) -> Result<(), AppError>;
    async fn list_template_versions(&self, template_id: &str) -> Result<Vec<EmailTemplateVersion>, AppError>;
    async fn get_template_version(&self, version_id: &str) -> Result<Option<EmailTemplateVersion>, AppError>;

    async fn create_rule(&self, rule: &NotificationRule) -> Result<NotificationRule, AppError>;
    async fn get_rules_by_event(&self, event_id: &str) -> Result<Vec<NotificationRule>, AppError>;
    async fn get_rules_by_trigger(&self, tenant_id: &str, event_id: Option<&str>, trigger: &str) -> Result<Vec<NotificationRule>, AppError>;
    async fn delete_rule(&self, id: &str) -> Result<(), AppError>;

    async fn log_mail(&self, log: &MailLog) -> Result<(), AppError>;
    async fn has_mail_been_sent(&self, recipient: &str, template_id: &str, context_hash: &str) -> Result<bool, AppError>;
    async fn list_logs(&self, tenant_id: &str, recipient: Option<&str>) -> Result<Vec<MailLog>, AppError>;
}