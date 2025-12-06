use crate::domain::ports::LlmService;
use crate::error::AppError;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tracing::{error, info, warn, instrument};
use std::time::Duration;
use tokio::time::sleep;

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

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
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    async fn send_request_with_retry(&self, url: &str, api_key: &str, payload: &Value) -> Result<String, AppError> {
        let mut retries = 0;
        let mut backoff = INITIAL_BACKOFF_MS;

        loop {
            let res = self.client.post(url)
                .header("x-goog-api-key", api_key)
                .header("Content-Type", "application/json")
                .json(payload)
                .send()
                .await;

            match res {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let body: Value = response.json().await.map_err(|e| {
                            error!("Failed to parse Gemini response JSON: {:?}", e);
                            AppError::Internal
                        })?;
                        return self.extract_content(body);
                    } else if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                        if retries >= MAX_RETRIES {
                            error!("Gemini API failed after {} retries. Status: {}", retries, status);
                            let text = response.text().await.unwrap_or_default();
                            return Err(AppError::InternalWithMsg(format!("AI Provider Error: {} - {}", status, text)));
                        }
                        warn!("Gemini API transient error {}. Retrying in {}ms...", status, backoff);
                    } else {
                        let text = response.text().await.unwrap_or_default();
                        error!("Gemini API Terminal Error {}: {}", status, text);
                        return Err(AppError::Validation(format!("AI Request Rejected: {} - {}", status, text)));
                    }
                },
                Err(e) => {
                    if retries >= MAX_RETRIES {
                        error!("Gemini Network Error after {} retries: {:?}", retries, e);
                        return Err(AppError::InternalWithMsg(format!("AI Network Error: {}", e)));
                    }
                    warn!("Gemini Network Error. Retrying in {}ms... {:?}", backoff, e);
                }
            }

            sleep(Duration::from_millis(backoff)).await;
            retries += 1;
            backoff *= 2;
        }
    }

    fn extract_content(&self, body: Value) -> Result<String, AppError> {
        if let Some(candidates) = body.get("candidates").and_then(|c| c.as_array())
            && let Some(first) = candidates.first() {

            if let Some(finish_reason) = first.get("finishReason").and_then(|s| s.as_str())
                && finish_reason != "STOP" {
                    warn!("AI generation stopped abnormally. Reason: {}", finish_reason);
                    if finish_reason == "SAFETY" {
                        return Err(AppError::Validation("AI content generation blocked by safety filters.".to_string()));
                    }
                }

            if let Some(content) = first.get("content")
                && let Some(parts) = content.get("parts").and_then(|p| p.as_array())
                && let Some(text_part) = parts.first()
                && let Some(text) = text_part.get("text").and_then(|t| t.as_str()) {
                // Clean up markdown code fences if present
                let cleaned_text = text.trim()
                    .trim_start_matches("```html")
                    .trim_start_matches("```mjml")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                return Ok(cleaned_text.to_string());
            }
        }

        error!("Invalid or unexpected response structure from Gemini: {:?}", body);
        Err(AppError::InternalWithMsg("AI response missing content".to_string()))
    }
}

#[async_trait]
impl LlmService for GeminiService {
    #[instrument(skip(self, api_key), fields(prompt_len = prompt.len()))]
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
                "temperature": 1.0,
                "maxOutputTokens": 4000,
                "topP": 0.8,
                "topK": 40
            },
            "safetySettings": [
                {
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_HATE_SPEECH",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                },
                {
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "threshold": "BLOCK_MEDIUM_AND_ABOVE"
                }
            ]
        });

        info!("Sending generation request to Gemini...");
        let result = self.send_request_with_retry(url, api_key, &payload).await?;
        info!("Successfully generated content from AI.");
        Ok(result)
    }
}