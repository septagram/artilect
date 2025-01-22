use dioxus::prelude::*;
use uuid::Uuid;

use super::SidebarThreadLink;
use crate::state::State;
use crate::Route;

pub static CSS: Asset = asset!("/src/components/layout.css");

#[component]
pub fn Layout() -> Element {
    let state = use_context::<State>();
    let thread_ids: Vec<Uuid> = state.thread_list.read().clone();

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
