use dioxus::logger::tracing::Level;
use dioxus::prelude::*;

use artilect::actuators::chat::front::App;

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
        .launch(App);
}
