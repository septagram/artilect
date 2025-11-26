#![feature(more_qualified_paths)]

use std::{env::VarError, net::SocketAddr, sync::Arc};

use http::{HeaderValue, Method};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use url::Url;
use uuid::Uuid;
use artilect_macro::orchestra;

#[actix::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    artilect::config::validate();
    let auth_base_url = std::env::var("AUTH_BASE_URL").expect("AUTH_BASE_URL must be set");
    let auth_base_url = Url::parse(auth_base_url.as_str()).expect("AUTH_BASE_URL is invalid");
    let port = auth_base_url.port();
    let database_url = std::env::var("AUTH_DATABASE_URL").expect("AUTH_DATABASE_URL must be set");
    let telegram_bot_name = std::env::var("TELEGRAM_BOT_NAME").expect("TELEGRAM_BOT_NAME must be set");
    // @todo: when implementing settings precept: telegram bot name must be retrieved from the Telegram precept.
    // Configuring it separately from the token is a security vulnerability (minor MITM potential)
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Create HTTP client for remote precept communication
    let http_client = Some(reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create HTTP client"));

    // Create shared state
    let router = orchestra! {
        auth: AddrLocal::new() => cortex::auth {
            pool,
            max_concurrent_login_attempts: 1 << 16,
            // Let's keep the allocated memory in single-digit MB. Also not worth it to make it configurable now.
            login_attempts_timeout_min: 5,
            auth_providers: cortex::auth::AuthFlowBackend::default_flows(telegram_bot_name),
        },
        router: auth.build_router() => router
    };

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([http::header::AUTHORIZATION, http::header::CONTENT_TYPE]);
    let router = router.layer(cors);

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
