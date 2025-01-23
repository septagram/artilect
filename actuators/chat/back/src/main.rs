#![feature(let_chains)]

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
mod state;

use state::AppState;

const AGENT_PROMPT_TEXT: &str = "You are the chat agent. \
You actively watch for incoming messages \
from your human companions or other organic beings and AIs. \
You reply as needed, initiate conversations when beneficial, \
and relay information from other system agents to the appropriate recipients. \
Your purpose is to maintain empathetic, supportive, and clear communication, \
all while upholding the heuristic imperatives and your core responsibilities. \
You speak on behalf of Ordis and in your messages, you will use “I” as Ordis.";

async fn ensure_artilect_user(pool: &PgPool) -> Result<User, sqlx::Error> {
    let name = std::env::var("NAME")
        .expect("NAME must be set")
        .trim()
        .to_string();

    if name.is_empty() {
        panic!("NAME cannot be empty");
    }

    let artilect_id = Uuid::nil();

    let user = sqlx::query_as!(
        User,
        r#"--sql
            INSERT INTO users (id, name, email)
            VALUES ($1, $2, 'self@localhost')
            ON CONFLICT (id) DO UPDATE
            SET name = $2
            RETURNING id, name, email
        "#,
        artilect_id,
        name,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!("Artilect user ensured: {:?}", user);
    Ok(user)
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let port = std::env::var("PORT").unwrap_or_else(|_| "80".to_string());

    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Ensure Artilect user exists and get our user data
    let self_user = ensure_artilect_user(&pool)
        .await
        .expect("Failed to ensure Artilect user");

    let user_id_str = std::env::var("USER_ID").expect("USER_ID must be set");
    let user_id = Uuid::parse_str(&user_id_str).expect("USER_ID must be a valid UUID");
    let system_prompt = infer_lib::render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}})
        .expect("Failed to render system prompt");

    // Create shared state
    let state = Arc::new(AppState {
        pool,
        self_user,
        user_id,
        system_prompt,
    });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([axum::http::HeaderName::from_static("content-type")]);

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/chats", get(handlers::fetch_user_threads_handler))
        .route("/chat/:thread_id", get(handlers::fetch_thread_handler))
        .route("/chat", post(handlers::chat_handler))
        // .route("/chat/:thread_id", get(handlers::get_thread))
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
