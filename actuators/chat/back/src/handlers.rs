use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use chat_dto::{
    Message, OneToManyChild, OneToManyUpdate, SendMessageRequest, SendMessageResponse, SyncUpdate,
    Thread,
};

use crate::{
    models::{OpenAIMessage, OpenAIRequest},
    AppState,
};

pub async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn create_thread(
    pool: &PgPool,
    user_id: Uuid,
    thread_id: Uuid,
) -> Result<Thread, Box<dyn std::error::Error + Send + Sync>> {
    let mut tx = pool.begin().await?;

    // Create the thread
    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        INSERT INTO threads (id, name)
        VALUES ($1, NULL)
        RETURNING id, name, owner_id, created_at, updated_at
        "#,
        thread_id
    )
    .fetch_one(&mut *tx)
    .await?;

    // Add both users as participants
    sqlx::query!(
        r#"--sql
        INSERT INTO thread_participants (thread_id, user_id)
        VALUES ($1, $2), ($1, $3)
        "#,
        thread.id,
        user_id,
        Uuid::nil(),
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(thread)
}

// async fn save_message<'e, E>(executor: &mut E, user_id: Uuid, thread_id: Uuid, message: &str) -> Result<(), Box<dyn std::error::Error>>
// where
//     E: Executor<'e, Database = Postgres> + Copy,

async fn create_message(
    pool: &PgPool,
    user_id: Uuid,
    thread_id: Uuid,
    message_id: Option<Uuid>,
    message: &str,
) -> Result<(Message, Thread), Box<dyn std::error::Error + Send + Sync>> {
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

    let message = sqlx::query_as!(
        Message,
        r#"--sql
        INSERT INTO messages (id, user_id, thread_id, content)
        VALUES (COALESCE($1, gen_random_uuid()), $2, $3, $4)
        RETURNING id, thread_id, user_id, content, created_at, updated_at
        "#,
        message_id,
        user_id,
        thread_id,
        message
    )
    .fetch_one(&mut *tx)
    .await?;

    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        UPDATE threads SET updated_at = $1 WHERE id = $2
        RETURNING id, name, owner_id, created_at, updated_at
        "#,
        message.created_at,
        thread_id
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((message, thread))
}

async fn get_thread_message_ids(
    pool: &PgPool,
    thread_id: Uuid,
) -> Result<Vec<Uuid>, Box<dyn std::error::Error + Send + Sync>> {
    let messages = sqlx::query!(
        r#"--sql
        SELECT id FROM messages WHERE thread_id = $1
        "#,
        thread_id
    )
    .fetch_all(pool)
    .await?;
    Ok(messages.into_iter().map(|m| m.id).collect())
}

async fn respond_to_thread(
    state: &AppState,
    thread_id: Uuid,
) -> Result<(Message, Thread), Box<dyn std::error::Error + Send + Sync>> {
    #[derive(sqlx::FromRow)]
    struct MessageForAI {
        user_id: Uuid,
        content: String,
        created_at: time::OffsetDateTime,
    }

    let date_format = time::format_description::parse(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
         sign:mandatory]:[offset_minute]",
    )
    .expect("Failed to parse date format");

    let timezone = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);

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
        role: if msg.user_id == Uuid::nil() {
            "assistant"
        } else {
            "user"
        }
        .to_string(),
        content: format!(
            "[{}] {}",
            msg.created_at
                .to_offset(timezone)
                .format(&date_format)
                .expect("Failed to format date"),
            msg.content
        ),
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
        let (message, thread) =
            create_message(&state.pool, state.self_user.id, thread_id, None, &content).await?;
        Ok((message, thread))
    } else {
        Err("Invalid response format".into())
    }
}

async fn chat(
    state: &AppState,
    request: &SendMessageRequest,
) -> Result<SendMessageResponse, Box<dyn std::error::Error + Send + Sync>> {
    let thread_id = request.message.thread_id;
    if request.is_new_thread {
        create_thread(&state.pool, state.user_id, thread_id).await?;
    }
    let (user_message, _) = create_message(
        &state.pool,
        state.user_id,
        thread_id,
        Some(request.message.id),
        &request.message.content,
    )
    .await?;
    let (ai_message, thread) = respond_to_thread(&state, thread_id).await?;
    let threads = vec![SyncUpdate::Updated(thread)];
    let thread_messages = OneToManyUpdate {
        owner_id: thread_id,
        children: get_thread_message_ids(&state.pool, thread_id)
            .await?
            .into_iter()
            .map(|id| {
                if id == user_message.id {
                    OneToManyChild::Value(user_message.clone())
                } else if id == ai_message.id {
                    OneToManyChild::Value(ai_message.clone())
                } else {
                    OneToManyChild::Id(id)
                }
            })
            .collect::<Vec<_>>(),
    };
    Ok(SendMessageResponse {
        threads,
        thread_messages: vec![thread_messages],
    })
}

pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    match chat(&state, &request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            eprintln!("Error in chat handler: {:#?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}
