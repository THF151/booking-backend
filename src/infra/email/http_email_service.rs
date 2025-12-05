use crate::domain::ports::EmailService;
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::error;
use base64::{Engine as _, engine::general_purpose};

pub struct HttpEmailService {
    client: Client,
    api_url: String,
    api_key: String,
}

impl HttpEmailService {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_url,
            api_key,
        }
    }
}

#[derive(Serialize)]
struct AttachmentPayload {
    filename: String,
    content_base64: String,
}

#[derive(Serialize)]
struct EmailPayload {
    from_alias: String,
    to_addr: String,
    subject: String,
    html_body: String,
    attachments: Vec<AttachmentPayload>,
}

#[async_trait]
impl EmailService for HttpEmailService {
    async fn send(
        &self,
        recipient: &str,
        subject: &str,
        html_body: &str,
        attachment_name: Option<&str>,
        attachment_data: Option<&[u8]>
    ) -> Result<(), AppError> {
        let mut attachments = Vec::new();

        if let (Some(name), Some(data)) = (attachment_name, attachment_data) {
            let b64 = general_purpose::STANDARD.encode(data);
            attachments.push(AttachmentPayload {
                filename: name.to_string(),
                content_base64: b64,
            });
        }

        let payload = EmailPayload {
            from_alias: "default".to_string(),
            to_addr: recipient.to_string(),
            subject: subject.to_string(),
            html_body: html_body.to_string(),
            attachments,
        };

        let res = self.client.post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Email service connection error: {}", e);
                error!("{}", msg);
                AppError::InternalWithMsg(msg)
            })?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            let msg = format!("Email service failed. Status: {}, Body: {}", status, text);
            error!("{}", msg);
            return Err(AppError::InternalWithMsg(msg));
        }

        Ok(())
    }
}