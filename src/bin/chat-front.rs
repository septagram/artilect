use std::sync::Arc;

use artilect::{
    orchestra::{Orchestra, use_address_book},
    precepts::chat::{Addr, front::App},
};
use dioxus::{logger::tracing::Level, prelude::*};
use uuid::{Uuid, uuid};

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
            let orchestra = Arc::new(Orchestra {
                base_url: Arc::from(BASE_URL),
                chat: Addr::new(Arc::from(BASE_URL)),
            });
            use_address_book(orchestra, None);
            rsx! {
                App {}
            }
        });
}
