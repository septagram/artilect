use axum::{extract::State, response::IntoResponse, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{
    models::{ChatRequest, OpenAIMessage, OpenAIRequest},
    AppState,
};

pub async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

pub async fn chat(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Json<Value> {
    let client = reqwest::Client::new();

    let openai_request = OpenAIRequest {
        model: state.model.clone(),
        messages: vec![OpenAIMessage {
            role: "user".to_string(),
            content: request.message,
        }],
    };

    let response = client
        .post(format!("{}/v1/chat/completions", state.infer_url))
        .json(&openai_request)
        .send()
        .await;

    match response {
        Ok(res) => {
            match res.json::<Value>().await {
                Ok(json) => {
                    // Extract message from choices array
                    if let Some(choices) = json.get("choices")
                        && let Some(first_choice) = choices.as_array().and_then(|c| c.first())
                        && let Some(message) = first_choice.get("message")
                    {
                        return Json(json!({
                            "role": message.get("role").and_then(Value::as_str).unwrap_or("assistant"),
                            "content": message.get("content").and_then(Value::as_str).unwrap_or("")
                        }));
                    }
                    Json(json!({"error": "Invalid response format"}))
                }
                Err(_) => Json(json!({"error": "Failed to parse response"})),
            }
        }
        Err(_) => Json(json!({"error": "Failed to connect to infer service"})),
    }
}
