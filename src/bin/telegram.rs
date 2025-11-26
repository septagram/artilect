#![feature(more_qualified_paths)]

use std::sync::Arc;

use actix::Actor;
use artilect::precept::client::{AddrLocal, AddrRemote};
use artilect_macro::orchestra;
use uuid::Uuid;

#[actix::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    artilect::config::validate();

    // Create HTTP client for remote precept communication
    let http_client = Some(reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create HTTP client"));

    let _ = orchestra! {
        auth: AddrRemote::new(std::env::var("AUTH_BASE_URL").unwrap().into()),
        telegram: AddrLocal::new() => vector::telegram {
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN").unwrap().into(),
        },
    };

    tokio::signal::ctrl_c().await.unwrap();
    tracing::info!("Shutting down telegram service");
}
