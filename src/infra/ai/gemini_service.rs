use crate::domain::ports::LlmService;
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::error;

pub struct GeminiService {
    client: Client,
}

impl Default for GeminiService {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LlmService for GeminiService {
    async fn generate(
        &self,
        api_key: &str,
        prompt: &str,
        system_instruction: &str
    ) -> Result<String, AppError> {
        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

        let payload = json!({
            "contents": [{
                "parts": [{"text": prompt}]
            }],
            "systemInstruction": {
                "parts": [{"text": system_instruction}]
            },
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 2000
            }
        });

        let res = self.client.post(url)
            .header("x-goog-api-key", api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Gemini request failed: {:?}", e);
                AppError::InternalWithMsg(format!("AI Service unreachable: {}", e))
            })?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            error!("Gemini Error {}: {}", status, text);
            return Err(AppError::InternalWithMsg(format!("AI Provider Error: {}", status)));
        }

        let body: Value = res.json().await.map_err(|e| {
            error!("Failed to parse Gemini response: {:?}", e);
            AppError::Internal
        })?;

        if let Some(candidates) = body.get("candidates").and_then(|c| c.as_array())
            && let Some(first) = candidates.first()
                && let Some(content) = first.get("content")
                    && let Some(parts) = content.get("parts").and_then(|p| p.as_array())
                        && let Some(text_part) = parts.first()
                            && let Some(text) = text_part.get("text").and_then(|t| t.as_str()) {
                                return Ok(text.to_string());
                            }

        Err(AppError::InternalWithMsg("Empty or invalid response from AI".to_string()))
    }
}