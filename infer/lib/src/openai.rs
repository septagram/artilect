use serde::{Deserialize, Serialize};

use crate::InferError;
#[derive(Debug, Serialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIError {
    pub error: String,
}

pub async fn send_openai_request(
    messages: Vec<OpenAIMessage>,
    model: String,
    infer_url: String,
) -> Result<String, InferError> {
    let openai_request = OpenAIRequest {
        model,
        messages,
    };

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
        return Err(InferError::from(error_response));
    }

    // If not error, parse as success response
    let response: OpenAIResponse = serde_json::from_str(&response_text)?;
    Ok(response
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_default())
} 