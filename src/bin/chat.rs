use std::{env::VarError, net::SocketAddr, sync::Arc};

use actix::Actor;
use artilect::{
    infer::RootChain,
    orchestra::Orchestra,
    precept::{Identity, PreceptID, Routable, client::AddrLocal},
    precepts::{
        cortex::auth::middleware::RouterAuth,
        vector::chat::{AGENT_PROMPT_TEXT, Precept as ChatPrecept, ensure_artilect_user},
    },
};
use http::{HeaderValue, Method};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use uuid::Uuid;
use artilect::precepts::cortex::auth;

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
    let infer_client = artilect::infer::Client::new();

    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Ensure Artilect user exists and get our user data
    let self_user = ensure_artilect_user(&pool, name)
        .await
        .expect("Failed to ensure Artilect user");

    let system_prompt =
        RootChain::from_message(infer_client, artilect::prompts::system(AGENT_PROMPT_TEXT));

    // Create shared state
    let router = {
        let (chat, chat_addr) = AddrLocal::new();
        let orchestra = Orchestra { chat };
        let chat_actor = ChatPrecept::new(
            orchestra.to_address_book(
                Some(Identity::Precept {
                    id: PreceptID::Chat,
                    on_behalf_of: None,
                }),
                None,
            ),
            pool,
            self_user,
            system_prompt,
        )
        .start();
        let router = chat_actor.clone().build_router().require_access_token();
        chat_addr.set(chat_actor).unwrap();
        router
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
