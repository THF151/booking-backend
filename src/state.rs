use std::sync::Arc;
use crate::domain::ports::{
    BookingRepository, EventRepository, InviteeRepository, TenantRepository,
    UserRepository, JobRepository, EmailService, EventOverrideRepository,
    AuthRepository, BookingLabelRepository, SessionRepository, CommunicationRepository,
    LlmService
};
use crate::domain::services::auth_service::AuthService;
use crate::config::Config;
use tera::Tera;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub tenant_repo: Arc<dyn TenantRepository>,
    pub user_repo: Arc<dyn UserRepository>,
    pub event_repo: Arc<dyn EventRepository>,
    pub booking_repo: Arc<dyn BookingRepository>,
    pub invitee_repo: Arc<dyn InviteeRepository>,
    pub job_repo: Arc<dyn JobRepository>,
    pub event_override_repo: Arc<dyn EventOverrideRepository>,
    pub auth_repo: Arc<dyn AuthRepository>,
    pub label_repo: Arc<dyn BookingLabelRepository>,
    pub session_repo: Arc<dyn SessionRepository>,
    pub communication_repo: Arc<dyn CommunicationRepository>,
    pub auth_service: Arc<AuthService>,
    pub email_service: Arc<dyn EmailService>,
    pub llm_service: Arc<dyn LlmService>,
    pub templates: Arc<Tera>,
}