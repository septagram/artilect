use std::sync::Arc;
use dioxus::logger::tracing::Level;
use dioxus::prelude::*;
use uuid::{uuid, Uuid};

use artilect::orchestra::{use_address_book, Orchestra};
use artilect::precepts::vector::chat::Addr;
use artilect::precepts::vector::chat::front::App;

static BASE_URL: &str = dotenvy_macro::dotenv!("CHAT_BASE_URL");
static USER_ID: Uuid = uuid!(dotenvy_macro::dotenv!("CHAT_USER_ID"));

fn main() {
    dioxus::logger::init(Level::INFO).unwrap();
    dioxus::LaunchBuilder::new()
        .with_cfg(desktop!({
            use dioxus::desktop::{Config, WindowBuilder};
            use tao::window::Theme;
            Config::new().with_menu(None).with_window(
                WindowBuilder::default()
                    .with_title("Artilect")
                    .with_maximized(true)
                    .with_theme(Some(Theme::Dark)),
            )
        }))
        .launch(|| {
            tracing::info!("Starting app");
            let orchestra = Arc::new(
                Orchestra {
                    chat: Addr::new(Arc::from(BASE_URL)),
                },
            );
            let token = Some(Arc::from(USER_ID.to_string()));
            tracing::info!("Using token: {:?}", token);
            use_address_book(orchestra, None, token);
            rsx!{
                App {}
            }
        });
}
