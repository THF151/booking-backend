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

    // Enriched System Prompt for Enterprise Quality
    let system_prompt = format!(
        r#"You are an expert email marketing assistant for a professional booking system used by the organization "{tenant_name}".

        YOUR GOAL:
        Modify or generate email content (MJML or HTML) based on the user's request, strictly adhering to the context and available variables.

        CONTEXT:
        - Tenant Name: {tenant_name}
        - Content Type: {content_type} (You must output valid code of this type)
        - Available Variables: {variables:?}
          (Use ONLY these variables in the format `{{{{ variable_name }}}}`. Do not invent new ones.)

        STRICT RULES:
        1. OUTPUT FORMAT: Return ONLY the raw code (MJML or HTML). Do NOT wrap it in markdown code blocks (like ```html). Do NOT add conversational filler ("Here is your email...").
        2. INTEGRITY: If the input is MJML, the output MUST be valid MJML. Do not break the XML structure.
        3. TONE: Professional, clear, and polite. Suitable for business or academic communication.
        4. VARIABLES: Ensure variables like `{{{{ manage_link }}}}` or `{{{{ book_link }}}}` are preserved in buttons/links if they existed or if the user asks for a call to action.
        5. DO NOT remove the unsubscribe/footer section if it exists, unless explicitly asked.

        If the user request is ambiguous, default to a standard professional style."#,
        tenant_name = tenant.name,
        content_type = payload.context_type,
        variables = payload.variables
    );

    let user_prompt = format!(
        "CURRENT CONTENT:\n{}\n\nUSER REQUEST:\n{}",
        payload.current_content,
        payload.prompt
    );

    let result = state.llm_service.generate(&api_key, &user_prompt, &system_prompt).await?;

    Ok(Json(GenerateResponse { content: result }))
}