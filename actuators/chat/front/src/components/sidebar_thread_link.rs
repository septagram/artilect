use dioxus::prelude::*;
use uuid::Uuid;

use crate::state::State;
use crate::Route;

#[component]
pub fn SidebarThreadLink(thread_id: Uuid) -> Element {
    let state = use_context::<State>();
    let route = use_route::<Route>();
    let is_active = matches!(route, Route::Chat { thread_id: current_id } if current_id == thread_id);
    
    let threads = state.threads.read();
    let thread_state = threads
        .get(&thread_id)
        .expect("Thread ID does not exist: {thread_id}");

    match thread_state.read() {
        None => rsx! {},
        Some(thread) => {
            let name = thread.name.as_deref().unwrap_or("Untitled Chat");
            rsx! {
                Link {
                    class: if is_active { "app__thread-link active" } else { "app__thread-link" },
                    to: Route::Chat { thread_id: thread.id },
                    "{name}"
                }
            }
        }
    }
}
