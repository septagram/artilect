use axum::{extract::State, response::IntoResponse, Json};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use chat_dto::{Message, SendMessageRequest, SendMessageResponse};

use crate::{
    models::{OpenAIMessage, OpenAIRequest},
    AppState,
};

pub async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn create_thread(pool: &PgPool, user_id: Uuid, self_user_id: Uuid) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
    let mut tx = pool.begin().await?;
    
    // Create the thread
    let thread_id = sqlx::query!(
        r#"--sql
        INSERT INTO threads DEFAULT VALUES
        RETURNING id
        "#
    )
    .fetch_one(&mut *tx)
    .await?
    .id;

    // Add both users as participants
    sqlx::query!(
        r#"--sql
        INSERT INTO thread_participants (thread_id, user_id)
        VALUES ($1, $2), ($1, $3)
        "#,
        thread_id,
        user_id,
        self_user_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(thread_id)
}

// async fn save_message<'e, E>(executor: &mut E, user_id: Uuid, thread_id: Uuid, message: &str) -> Result<(), Box<dyn std::error::Error>>
// where
//     E: Executor<'e, Database = Postgres> + Copy,

async fn create_message(
    pool: &PgPool,
    user_id: Uuid,
    thread_id: Uuid,
    message: &str,
) -> Result<(Uuid, time::OffsetDateTime), Box<dyn std::error::Error + Send + Sync>> {
    let mut tx = pool.begin().await?;
    let is_participant = sqlx::query!(
        r#"--sql
        SELECT EXISTS (
            SELECT 1 FROM thread_participants
            WHERE thread_id = $1 AND user_id = $2
        )
        "#,
        thread_id,
        user_id
    )
    .fetch_one(&mut *tx)
    .await?
    .exists;

    if !is_participant.unwrap_or(false) {
        return Err(Box::new(sqlx::Error::Protocol(
            "User is not a participant in this thread".into(),
        )));
    }

    let record = sqlx::query!(
        r#"--sql
        INSERT INTO messages (user_id, thread_id, content)
        VALUES ($1, $2, $3)
        RETURNING id, created_at
        "#,
        user_id,
        thread_id,
        message
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((record.id, record.created_at))
}

async fn respond_to_thread(
    state: &AppState,
    thread_id: Uuid,
) -> Result<Message, Box<dyn std::error::Error + Send + Sync>> {
    #[derive(sqlx::FromRow)]
    struct MessageForAI {
        user_id: Uuid,
        content: String, 
        created_at: time::OffsetDateTime,
    }

    let messages = sqlx::query_as!(
        MessageForAI,
        r#"--sql
            SELECT user_id, content, created_at
            FROM messages 
            WHERE thread_id = $1 
            ORDER BY created_at ASC
        "#,
        thread_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|msg| OpenAIMessage {
        role: if msg.user_id == state.user_id {
            "assistant"
        } else {
            "user"
        }
        .to_string(),
        content: msg.content,
    })
    .collect::<Vec<_>>();

    let openai_request = OpenAIRequest {
        model: state.model.clone(),
        messages: messages,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", state.infer_url))
        .json(&openai_request)
        .send()
        .await?;

    let json = response.json::<Value>().await?;
    if let Some(choices) = json.get("choices")
        && let Some(first_choice) = choices.as_array().and_then(|c| c.first())
        && let Some(message) = first_choice.get("message")
    {
        let content = message.get("content").and_then(Value::as_str).unwrap_or("");
        let (id, created_at) =
            create_message(&state.pool, state.self_user.id, thread_id, &content)
                .await?;

        // Create and return Message struct
        let message = Message {
            id,
            thread_id,
            user_id: state.self_user.id,
            content: content.to_string(),
            created_at,
            updated_at: None,
        };
        Ok(message)
    } else {
        Err("Invalid response format".into())
    }
}

async fn chat(state: &AppState, request: &SendMessageRequest) -> Result<Message, Box<dyn std::error::Error + Send + Sync>> {
    let thread_id = match request.thread_id {
        Some(thread_id) => thread_id,
        None => create_thread(&state.pool, state.user_id, state.self_user.id).await?,
    };
    create_message(&state.pool, state.user_id, thread_id, &request.message).await?;
    respond_to_thread(&state, thread_id).await
}

pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SendMessageRequest>,
) -> Json<Value> {
    match chat(&state, &request).await {
        Ok(message) => Json(json!(SendMessageResponse { message })),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}
