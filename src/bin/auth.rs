use std::{env::VarError, net::SocketAddr, sync::Arc};

use actix::Actor;
use artilect::{
    orchestra::Orchestra,
    precept::{Identity, PreceptID, Routable, client::AddrLocal},
    precepts::cortex::{
        auth,
        auth::{Precept as AuthPrecept, Resources as AuthResources, middleware::RouterAuth},
    },
};
use http::{HeaderValue, Method};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use url::Url;
use uuid::Uuid;

#[actix::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    artilect::config::validate();

    let auth_base_url = std::env::var("AUTH_BASE_URL").expect("AUTH_BASE_URL must be set");
    let auth_base_url = Url::parse(auth_base_url.as_str()).expect("AUTH_BASE_URL is invalid");
    let port = auth_base_url.port();
    let database_url = std::env::var("AUTH_DATABASE_URL").expect("AUTH_DATABASE_URL must be set");

    // Create database connection pool
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Create shared state
    let router = {
        let (auth, auth_addr) = AddrLocal::new();
        let orchestra = Orchestra { auth };
        let auth_actor = AuthPrecept::new(Arc::new(AuthResources {
            address_book: orchestra.to_address_book(
                Some(Identity::Precept {
                    id: PreceptID::Auth,
                    on_behalf_of: None,
                }),
                None,
            ),
            pool,
            max_concurrent_login_attempts: 1 << 20,
            // Around a million is fine probs, not worth it to make it configurable now.
            login_attempts_timeout_min: 5,
        }))
        .start();
        let router = auth_actor.clone().build_router();
        auth_addr.set(auth_actor).unwrap();
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
