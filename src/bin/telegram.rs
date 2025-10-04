use std::sync::Arc;
use actix::Actor;
use uuid::Uuid;

use artilect::precept::client::AddrLocal;
use artilect::precept::{Identity, PreceptID};
use artilect::precepts::vector::telegram::{Precept as TelegramPrecept, Resources as TelegramResources};

// Define a local Resources struct that matches the one in the telegram precept
use artilect::orchestra::Orchestra;

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

    // No database connection needed for telegram precept

    // Create shared state
    {
        let (telegram, telegram_addr) = AddrLocal::new();
        let orchestra = Orchestra { telegram };
        let address_book = orchestra.to_address_book(
            Some(Identity {
                user_id: Uuid::nil(),
                precept_id: Some(PreceptID::Telegram),
            }),
            None,
        );

        // Create resources for the telegram precept
        let resources = TelegramResources {
            address_book,
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN").unwrap().into(),
        };

        let telegram_actor = TelegramPrecept::new(resources).start();
        telegram_addr.set(telegram_actor).unwrap();
    }

    // Keep the application running
    tokio::signal::ctrl_c().await.unwrap();
    tracing::info!("Shutting down telegram service");
}
