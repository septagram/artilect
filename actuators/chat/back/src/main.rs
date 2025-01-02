#![feature(let_chains)]

use axum::{
    routing::{get, post},
    Router,
    http::{HeaderValue, Method},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

mod handlers;
mod models;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

#[derive(Clone)]
pub struct AppState {
    infer_url: String,
    model: String,
    pool: PgPool,
    self_user: User,
}

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
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let infer_url = std::env::var("INFER_URL")
        .unwrap_or_else(|_| "http://infer".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "80".to_string());
    let model = std::env::var("DEFAULT_MODEL")
        .unwrap_or_else(|_| "default".to_string());

    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Ensure Artilect user exists and get our user data
    let self_user = ensure_artilect_user(&pool)
        .await
        .expect("Failed to ensure Artilect user");

    // Create shared state
    let state = Arc::new(AppState {
        infer_url,
        model,
        pool,
        self_user,
    });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([axum::http::HeaderName::from_static("content-type")]);

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/chat", post(handlers::chat))
        .layer(cors)
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse::<u16>().expect("Invalid PORT")));
    tracing::info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service()
    )
    .await
    .unwrap();
} 