use axum::{extract::{State, Path, Query}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::domain::models::{communication::{EmailTemplate, NotificationRule, EmailTemplateVersion, TemplatePlaceholder}, job::Job};
use crate::error::AppError;
use std::sync::Arc;
use chrono::{Utc, Duration};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub subject_template: String,
    pub body_template: String,
    pub template_type: String,
    pub event_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub trigger_type: String,
    pub template_id: String,
}

#[derive(Deserialize)]
pub struct CampaignPreviewRequest {
    pub event_id: String,
    pub target_type: String,
    pub label_id: Option<String>,
    pub status_filter: Option<String>,
}

#[derive(Serialize)]
pub struct CampaignRecipient {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub status: String,
    pub received_mail: bool,
}

#[derive(Deserialize)]
pub struct CampaignSendRequest {
    pub template_id: String,
    pub recipients: Vec<String>,
    pub target_type: String,
}

#[derive(Deserialize)]
pub struct GetLogsParams {
    pub recipient: Option<String>,
}

#[derive(Deserialize)]
pub struct TestEmailRequest {
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

pub async fn list_templates(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let event_id = params.get("event_id").map(|s| s.as_str());
    let templates = state.communication_repo.list_templates(&tenant_id, event_id).await?;
    Ok(Json(templates))
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let template = EmailTemplate::new(
        tenant_id,
        payload.event_id,
        payload.name,
        payload.subject_template,
        payload.body_template,
        payload.template_type,
    );
    let created = state.communication_repo.create_template(&template).await?;

    let version = EmailTemplateVersion::new(
        created.id.clone(),
        created.subject_template.clone(),
        created.body_template.clone()
    );
    state.communication_repo.create_template_version(&version).await?;

    info!("Created email template: {}", created.id);
    Ok(Json(created))
}

pub async fn update_template(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, template_id)): Path<(String, String)>,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut template = state.communication_repo.get_template(&template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }

    let version = EmailTemplateVersion::new(
        template.id.clone(),
        payload.subject_template.clone(),
        payload.body_template.clone()
    );
    state.communication_repo.create_template_version(&version).await?;

    template.name = payload.name;
    template.subject_template = payload.subject_template;
    template.body_template = payload.body_template;
    template.template_type = payload.template_type;
    template.updated_at = Utc::now();

    let updated = state.communication_repo.update_template(&template).await?;
    Ok(Json(updated))
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, template_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let template = state.communication_repo.get_template(&template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }

    state.communication_repo.delete_template(&template_id).await?;
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn get_template(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, template_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let template = state.communication_repo.get_template(&template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }
    Ok(Json(template))
}

pub async fn list_versions(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, template_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let template = state.communication_repo.get_template(&template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }

    let versions = state.communication_repo.list_template_versions(&template_id).await?;
    Ok(Json(versions))
}

pub async fn restore_version(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, template_id, version_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let mut template = state.communication_repo.get_template(&template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }

    let version = state.communication_repo.get_template_version(&version_id).await?
        .ok_or(AppError::NotFound("Version not found".into()))?;

    if version.template_id != template_id {
        return Err(AppError::Validation("Version does not belong to this template".into()));
    }

    template.subject_template = version.subject_template;
    template.body_template = version.body_template;
    if template.body_template.trim().to_lowercase().starts_with("<mjml") {
        template.template_type = "mjml".to_string();
    } else {
        template.template_type = "html".to_string();
    }
    template.updated_at = Utc::now();

    let updated = state.communication_repo.update_template(&template).await?;

    let new_version = EmailTemplateVersion::new(
        template.id.clone(),
        template.subject_template.clone(),
        template.body_template.clone()
    );
    state.communication_repo.create_template_version(&new_version).await?;

    Ok(Json(updated))
}

pub async fn get_placeholders() -> impl IntoResponse {
    let placeholders = vec![
        TemplatePlaceholder { key: "user_name".to_string(), description: "Customer Name".to_string(), sample_value: "John Doe".to_string() },
        TemplatePlaceholder { key: "event_title".to_string(), description: "Event Title".to_string(), sample_value: "Consultation Call".to_string() },
        TemplatePlaceholder { key: "start_time".to_string(), description: "Booking Start Time".to_string(), sample_value: "2023-10-15 14:00".to_string() },
        TemplatePlaceholder { key: "location".to_string(), description: "Event Location".to_string(), sample_value: "Zoom Meeting".to_string() },
        TemplatePlaceholder { key: "duration".to_string(), description: "Duration (min)".to_string(), sample_value: "30".to_string() },
        TemplatePlaceholder { key: "manage_link".to_string(), description: "Link to manage booking".to_string(), sample_value: "https://example.com/manage/123".to_string() },
        TemplatePlaceholder { key: "token".to_string(), description: "Invitee Token".to_string(), sample_value: "abc-123-xyz".to_string() },
        TemplatePlaceholder { key: "book_link".to_string(), description: "Direct booking link (Invite)".to_string(), sample_value: "https://example.com/book?token=abc".to_string() },
    ];
    Json(placeholders)
}

pub async fn send_test_email(
    State(state): State<Arc<AppState>>,
    TenantId(_tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<TestEmailRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.email_service.send(
        &payload.recipient,
        &payload.subject,
        &payload.body,
        None,
        None
    ).await?;
    Ok(Json(serde_json::json!({"status": "sent"})))
}

pub async fn list_event_rules(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, event_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_id(&tenant_id, &event_id).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;
    let rules = state.communication_repo.get_rules_by_event(&event.id).await?;
    Ok(Json(rules))
}

pub async fn create_event_rule(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, event_id)): Path<(String, String)>,
    Json(payload): Json<CreateRuleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_id(&tenant_id, &event_id).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;
    let rule = NotificationRule::new(
        tenant_id,
        Some(event.id),
        payload.trigger_type,
        payload.template_id
    );
    let created = state.communication_repo.create_rule(&rule).await?;
    Ok(Json(created))
}

pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    TenantId(_tenant_id): TenantId,
    _user: AuthUser,
    Path((_, rule_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    state.communication_repo.delete_rule(&rule_id).await?;
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let jobs = state.job_repo.list_jobs(&tenant_id).await?;
    Ok(Json(jobs))
}

pub async fn preview_campaign(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<CampaignPreviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut result = Vec::new();

    if payload.target_type == "BOOKING" {
        let bookings = state.booking_repo.list_by_event(&tenant_id, &payload.event_id).await?;
        for b in bookings {
            if let Some(filter) = &payload.status_filter
                && &b.status != filter { continue; }
            if let Some(lbl) = &payload.label_id
                && b.label_id.as_deref() != Some(lbl) { continue; }
            result.push(CampaignRecipient {
                id: b.id,
                email: b.customer_email,
                name: Some(b.customer_name),
                status: b.status,
                received_mail: false
            });
        }
    } else if payload.target_type == "INVITEE" {
        let invitees = state.invitee_repo.list_by_event(&tenant_id, &payload.event_id).await?;
        for i in invitees {
            if let Some(filter) = &payload.status_filter
                && &i.status != filter { continue; }
            if let Some(email) = i.email {
                result.push(CampaignRecipient {
                    id: i.id,
                    email,
                    name: None,
                    status: i.status,
                    received_mail: false
                });
            }
        }
    }

    Ok(Json(result))
}

pub async fn send_campaign(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<CampaignSendRequest>,
) -> Result<impl IntoResponse, AppError> {
    let template = state.communication_repo.get_template(&payload.template_id).await?
        .ok_or(AppError::NotFound("Template not found".into()))?;

    if template.tenant_id != tenant_id {
        return Err(AppError::NotFound("Template not found".into()));
    }

    let mut count = 0;
    let mut delay_offset = 0;

    for recipient_id in payload.recipients {
        let type_prefix = format!("CAMPAIGN:{}:{}", payload.target_type, payload.template_id);

        let execute_at = Utc::now() + Duration::seconds(delay_offset as i64);
        delay_offset += 5;

        let job = Job::new(&type_prefix, recipient_id, tenant_id.clone(), execute_at);
        state.job_repo.create(&job).await?;
        count += 1;
    }

    Ok(Json(serde_json::json!({"status": "queued", "count": count})))
}

pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Query(params): Query<GetLogsParams>,
) -> Result<impl IntoResponse, AppError> {
    let logs = state.communication_repo.list_logs(&tenant_id, params.recipient.as_deref()).await?;
    Ok(Json(logs))
}