use dioxus::prelude::*;
use uuid::Uuid;

use super::{Chat, SidebarThreadLink};
use crate::state::State;
use crate::Route;

pub static CSS: Asset = asset!("/src/components/layout.css");

#[component]
pub fn Layout() -> Element {
    let state = use_context::<State>();
    let thread_ids: Vec<_> = state.threads.read()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    rsx! {
        div { class: "app",
            div { class: "app__sidebar",
                Link {
                    class: "app__new-chat",
                    to: Route::NewChat {},
                    "New Chat"
                }
                div { class: "app__thread-list",
                    for thread_id in thread_ids {
                        SidebarThreadLink {
                            key: "{thread_id}",
                            thread_id: thread_id,
                        }
                    }
                }
            }
            Outlet::<Route> {}
        }
    }
}
