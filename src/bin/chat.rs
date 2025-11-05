#![feature(more_qualified_paths)]

use std::{env::VarError, net::SocketAddr, sync::Arc};

use actix::Actor;
use artilect::infer::RootChain;
use http::{HeaderValue, Method};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use url::Url;
use uuid::Uuid;
use artilect::precepts::vector::chat::{ensure_artilect_user, AGENT_PROMPT_TEXT};
use artilect_macro::orchestra;

#[actix::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    artilect::config::validate();
    let chat_base_url = std::env::var("CHAT_BASE_URL").expect("CHAT_BASE_URL must be set");
    let chat_base_url = Url::parse(chat_base_url.as_str()).expect("CHAT_BASE_URL is invalid");
    let port = chat_base_url.port();
    let database_url = std::env::var("CHAT_DATABASE_URL").expect("CHAT_DATABASE_URL must be set");
    let infer_client = artilect::infer::Client::new();
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    let name = &**artilect::config::back_shared::NAME;
    let self_user = ensure_artilect_user(&pool, name)
        .await
        .expect("Failed to ensure Artilect user");
    let system_prompt =
        RootChain::from_message(infer_client, artilect::prompts::system(AGENT_PROMPT_TEXT));

    let router = orchestra! {
        chat: AddrLocal::new() => vector::chat {
            pool,
            self_user,
            system_prompt,
        },
        router: chat.build_router().require_access_token() => router,
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
