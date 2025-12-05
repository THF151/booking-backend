use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct EmailTemplate {
    pub id: String,
    pub tenant_id: String,
    pub event_id: Option<String>,
    pub name: String,
    pub subject_template: String,
    pub body_template: String,
    pub template_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl EmailTemplate {
    pub fn new(tenant_id: String, event_id: Option<String>, name: String, subject: String, body: String, t_type: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id,
            event_id,
            name,
            subject_template: subject,
            body_template: body,
            template_type: t_type,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct EmailTemplateVersion {
    pub id: String,
    pub template_id: String,
    pub subject_template: String,
    pub body_template: String,
    pub created_at: DateTime<Utc>,
}

impl EmailTemplateVersion {
    pub fn new(template_id: String, subject: String, body: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            template_id,
            subject_template: subject,
            body_template: body,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct NotificationRule {
    pub id: String,
    pub tenant_id: String,
    pub event_id: Option<String>,
    pub trigger_type: String,
    pub template_id: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl NotificationRule {
    pub fn new(tenant_id: String, event_id: Option<String>, trigger: String, template_id: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tenant_id,
            event_id,
            trigger_type: trigger,
            template_id,
            is_active: true,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct MailLog {
    pub id: String,
    pub job_id: String,
    pub recipient: String,
    pub template_id: String,
    pub context_hash: String,
    pub sent_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplatePlaceholder {
    pub key: String,
    pub description: String,
    pub sample_value: String,
}