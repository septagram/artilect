use dioxus::prelude::*;

mod chat;
mod chat_message;
mod layout;
mod sidebar_thread_link;

pub use chat::{Chat, NewChat};
pub use chat_message::ChatMessage;
pub use layout::Layout;
pub use sidebar_thread_link::SidebarThreadLink;

#[component]
pub fn Style() -> Element {
    rsx! {
        document::Stylesheet { href: chat::CSS }
        document::Stylesheet { href: chat_message::CSS }
        document::Stylesheet { href: layout::CSS }
    }
}
