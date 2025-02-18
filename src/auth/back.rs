#![feature(let_chains)]

use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    dotenvy::dotenv().ok();
    let database_url = std::env::var("CHAT_DATABASE_URL").expect("DATABASE_URL must be set");
    let port = std::env::var("PORT").unwrap_or_else(|_| "80".to_string());

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([http::header::AUTHORIZATION, http::header::CONTENT_TYPE]);
    
    Ok(())
}
