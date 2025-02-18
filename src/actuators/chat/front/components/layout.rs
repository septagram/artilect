use dioxus::prelude::*;
use uuid::Uuid;
// use tokio::time::{sleep, Duration};

use super::SidebarThreadLink;
use crate::state::State;
use crate::Route;

pub static CSS: Asset = asset!("/src/components/layout.css");

#[component]
pub fn Layout() -> Element {
    let b = classnames::classname("app");
    let state = use_context::<State>();
    let thread_ids: Vec<Uuid> = state.thread_list.read().clone();
    // let mut has_recently_scrolled = use_signal(|| false);
    // let toggle_recently_scrolled = use_coroutine(move |mut rx: UnboundedReceiver<()>| async move {
    //     // let mut disable_recently_scrolled = std::future::pending();
    //     spawn(|| {});
    //     loop {
    //         match std::future::select(disable_recently_scrolled, rx.next()).await {
    //             Either::Left(_) => {
    //                 has_recently_scrolled.set(false);
    //                 disable_recently_scrolled = std::future::pending();
    //             }
    //             Either::Right(Some(_)) => {
    //                 has_recently_scrolled.set(true);
    //                 disable_recently_scrolled = sleep(Duration::from_msecs(200)).await;
    //             }
    //         }
    //     }
    // });

    rsx! {
        div { class: b.to_string(),
            div { class: b.el("sidebar").to_string(),
                Link {
                    class: b.el("new-chat").to_string(),
                    to: Route::NewChat {},
                    "New Chat"
                }
                div { class: b.el("thread-list").maybe_attr("recently-scrolled", false /* *has_recently_scrolled.read() */).to_string(),
                    // onscroll: move |evt| {
                    //     has_recently_scrolled.set(true);
                    // },
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
