use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ROLE_SYSTEM: &str = "system";
pub const ROLE_USER: &str = "user";
pub const ROLE_ASSISTANT: &str = "assistant";

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Response parsing failed: {0}")]
    ParseFailed(#[from] serde_json::Error),

    #[error("Error response from API: {0}")]
    ErrorResponse(String),
}

impl From<OpenAIError> for ApiError {
    fn from(err: OpenAIError) -> Self {
        ApiError::ErrorResponse(err.error)
    }
}

#[derive(Debug, Serialize)]
pub struct OpenAIRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [OpenAIMessage],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: &'static str,
    pub content: Box<str>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIResponseMessage {
    pub content: Box<str>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIError {
    pub error: String,
}

pub async fn openai_request(
    messages: &[OpenAIMessage],
    model: &str,
    infer_url: &str,
) -> Result<Box<str>, ApiError> {
    let openai_request = OpenAIRequest { model, messages };

    let client = reqwest::Client::new();
    let response_text = client
        .post(format!("{}/v1/chat/completions", infer_url))
        .json(&openai_request)
        .send()
        .await?
        .text()
        .await?;

    // Try parsing as error response first
    if let Ok(error_response) = serde_json::from_str::<OpenAIError>(&response_text) {
        return Err(ApiError::from(error_response));
    }

    // If not error, parse as success response
    let response: OpenAIResponse = serde_json::from_str(&response_text)?;
    Ok(response
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_default())
}
