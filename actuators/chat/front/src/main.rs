#![feature(let_chains)]

use dioxus::logger::tracing::{info, Level};
use dioxus::prelude::*;
use uuid::Uuid;

mod api;
mod components;
mod state;
use components::{Chat, Layout, NewChat, Style};
use state::actions::FetchUserThreadsAction;
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Layout)]
    #[route("/")]
    NewChat {},
    #[route("/chat/:thread_id")]
    Chat { thread_id: Uuid },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    dioxus_logger::init(Level::INFO).unwrap();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    state::use_app_state();
    state::actions::use_app_actions();
    let dispatch_fetch_user_threads = use_coroutine_handle::<FetchUserThreadsAction>();
    use_effect(move || {
        info!("Fetching user threads...");
        dispatch_fetch_user_threads.send(());
    });
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        Style {}
        Router::<Route> {}
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::NewChat {},
                "New Chat"
            }
        }

        Outlet::<Route> {}
    }
}
