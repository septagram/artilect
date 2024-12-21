use axum::{
    Json,
    extract::State,
    response::IntoResponse,
};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, models::{ChatRequest, OpenAIRequest, OpenAIMessage}};

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
                Ok(json) => Json(json),
                Err(_) => Json(json!({"error": "Failed to parse response"})),
            }
        },
        Err(_) => Json(json!({"error": "Failed to connect to infer service"})),
    }
} 