use std::sync::Arc;
use crate::domain::{
    models::{communication::MailLog},
    ports::CommunicationRepository
};
use crate::error::AppError;
use tera::{Tera, Context};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use chrono::Utc;
use serde_json::Value;
use tracing::info;

pub struct CommunicationService {
    repo: Arc<dyn CommunicationRepository>,
}

impl CommunicationService {
    pub fn new(repo: Arc<dyn CommunicationRepository>) -> Self {
        Self { repo }
    }

    pub async fn render_and_log(
        &self,
        job_id: &str,
        recipient: &str,
        template_name: &str,
        tera: &Tera,
        context_data: &Value
    ) -> Result<(String, String, bool), AppError> {
        let context_json = serde_json::to_string(context_data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(template_name.as_bytes());
        hasher.update(context_json.as_bytes());
        let hash = hex::encode(hasher.finalize());

        if self.repo.has_mail_been_sent(recipient, template_name, &hash).await? {
            let log = MailLog {
                id: Uuid::new_v4().to_string(),
                job_id: job_id.to_string(),
                recipient: recipient.to_string(),
                template_id: template_name.to_string(),
                context_hash: hash,
                sent_at: Utc::now(),
                status: "SKIPPED_DUPLICATE".to_string(),
            };
            self.repo.log_mail(&log).await?;
            return Ok((String::new(), String::new(), true)); // Skipped
        }

        let context = Context::from_value(context_data.clone()).map_err(|_| AppError::Internal)?;

        let body = tera.render(template_name, &context)
            .map_err(|_| AppError::Internal)?;

        let subject = if template_name.contains("confirmation") {
            "Booking Confirmation"
        } else if template_name.contains("reminder") {
            "Reminder"
        } else {
            "Notification"
        };

        Ok((subject.to_string(), body, false))
    }

    pub async fn record_success(&self, job_id: &str, recipient: &str, template_name: &str, context_data: &Value) -> Result<(), AppError> {
        let context_json = serde_json::to_string(context_data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(template_name.as_bytes());
        hasher.update(context_json.as_bytes());
        let hash = hex::encode(hasher.finalize());

        info!("Recording success in Ledger for: {} (Template: {})", recipient, template_name);

        let log = MailLog {
            id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            recipient: recipient.to_string(),
            template_id: template_name.to_string(),
            context_hash: hash,
            sent_at: Utc::now(),
            status: "SENT".to_string(),
        };
        self.repo.log_mail(&log).await?;
        Ok(())
    }
}