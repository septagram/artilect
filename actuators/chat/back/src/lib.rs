#![feature(let_chains)]

use actix::Actor;
use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use dioxus::prelude::*;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use chat_dto::User;

mod components;
mod handlers;
mod actor;

use actor::ChatService;

const AGENT_PROMPT_TEXT: &str = "You are the chat agent. \
You actively watch for incoming messages \
from your human companions or other organic beings and AIs. \
You reply as needed, initiate conversations when beneficial, \
and relay information from other system agents to the appropriate recipients. \
Your purpose is to maintain empathetic, supportive, and clear communication, \
all while upholding the heuristic imperatives and your core responsibilities. \
You speak on behalf of Ordis and in your messages, you will use “I” as Ordis.";

async fn ensure_artilect_user(pool: &PgPool, name: Box<str>) -> Result<User, sqlx::Error> {
    let artilect_id = Uuid::nil();

    let user = sqlx::query_as!(
        User,
        r#"--sql
            INSERT INTO users (id, name)
            VALUES ($1, $2)
            ON CONFLICT (id) DO UPDATE
            SET name = $2
            RETURNING id, name
        "#,
        artilect_id,
        &name,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!("Artilect user ensured: {:?}", user);
    Ok(user)
}

pub async fn serve(name: Box<str>, database_url: Box<str>, port: Option<u16>) {
    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Ensure Artilect user exists and get our user data
    let self_user = ensure_artilect_user(&pool, name)
        .await
        .expect("Failed to ensure Artilect user");

    let system_prompt = infer::render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}})
        .expect("Failed to render system prompt");

    // Create shared state
    let actor = ChatService::new(pool, self_user, system_prompt).start();
    let state = Arc::new(actor.clone());

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([http::header::AUTHORIZATION, http::header::CONTENT_TYPE]);

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/chats", get(handlers::fetch_user_threads_handler))
        .route("/chat/{thread_id}", get(handlers::fetch_thread_handler))
        .route("/chat", post(handlers::chat_handler))
        .layer(cors)
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse::<u16>().expect("Invalid PORT")));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
