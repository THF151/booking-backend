use axum::{extract::State, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::error::AppError;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub current_content: String,
    pub context_type: String, // TEMPLATE or ADHOC
    pub variables: Vec<String>,
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub content: String,
}

pub async fn generate_content(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Json(payload): Json<GenerateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let tenant = state.tenant_repo.find_by_id(&tenant_id).await?
        .ok_or(AppError::NotFound("Tenant not found".into()))?;

    let api_key = tenant.ai_api_key.ok_or(AppError::Validation("AI API Key not configured for this tenant".into()))?;

    let system_prompt = format!(
        r#"You are an expert email marketing assistant for a booking system.
        Your task is to modify or generate email content based on the user's request.

        CONTEXT:
        - Tenant Name: {}
        - Content Type: {} (MJML or HTML email body)

        RULES:
        1. Output ONLY the resulting content (MJML or HTML). Do not include markdown code blocks or explanations.
        2. You MUST preserve the structure valid for MJML if the input looks like MJML.
        3. You MUST use the following variables correctly where appropriate:
           {:?}
           (Do not invent new variables. Use {{ variable_name }} syntax).
        4. Keep the tone professional and helpful.
        "#,
        tenant.name,
        payload.context_type,
        payload.variables
    );

    let user_prompt = format!(
        "Current Content:\n{}\n\nUser Request: {}",
        payload.current_content,
        payload.prompt
    );

    let result = state.llm_service.generate(&api_key, &user_prompt, &system_prompt).await?;

    Ok(Json(GenerateResponse { content: result }))
}