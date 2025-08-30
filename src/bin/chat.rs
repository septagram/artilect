#![feature(let_chains)]

use std::env::VarError;
use std::net::SocketAddr;
use std::sync::Arc;
use actix::Actor;
use sqlx::PgPool;
use artilect::infer::RootChain;
use artilect::precept::Routable;
use artilect::precepts::vector::chat::{
    ensure_artilect_user,
    AGENT_PROMPT_TEXT,
    Precept as ChatPrecept
};

#[actix::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    dotenvy::dotenv().ok();
    artilect::config::validate();
    let name: Box<str> = std::env::var("NAME")
        .expect("NAME must be set")
        .trim()
        .into();

    if name.is_empty() {
        panic!("NAME cannot be empty");
    }

    let database_url = std::env::var("CHAT_DATABASE_URL").expect("DATABASE_URL must be set");
    let port = match std::env::var("PORT") {
        Ok(port) => Some(port.parse::<u16>().expect("Invalid PORT")),
        Err(VarError::NotPresent) => None,
        Err(err) => panic!("Failed to parse PORT: {}", err),
    };
    let client = artilect::infer::Client::new();

    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Ensure Artilect user exists and get our user data
    let self_user = ensure_artilect_user(&pool, name)
        .await
        .expect("Failed to ensure Artilect user");

    let system_prompt = RootChain::from_message(client, artilect::prompts::system(AGENT_PROMPT_TEXT));

    // Create shared state
    let actor = ChatPrecept::new(pool, self_user, system_prompt).start();
    let state = Arc::new(actor.clone());

    let router = actor.build_router();

    // Start server
    if let Some(port) = port {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("Starting server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, router.into_make_service())
            .await
            .unwrap();
    } else {
        tracing::info!("Port not present, not starting the server.")
    }
}
