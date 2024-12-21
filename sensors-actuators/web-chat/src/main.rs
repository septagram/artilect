use axum::{
    routing::{get, post},
    Router,
    http::{HeaderValue, Method},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use std::sync::Arc;

mod handlers;
mod models;

#[derive(Clone)]
pub struct AppState {
    infer_url: String,
    model: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    dotenv::dotenv().ok();
    let infer_url = std::env::var("INFER_URL")
        .unwrap_or_else(|_| "http://infer".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "80".to_string());
    let model = std::env::var("DEFAULT_MODEL")
        .unwrap_or_else(|_| "default".to_string());

    // Create shared state
    let state = Arc::new(AppState {
        infer_url,
        model,
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