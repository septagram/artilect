use dioxus::prelude::*;

mod chat;
mod chat_message;

pub use chat::Chat;
pub use chat_message::ChatMessage;

pub fn Style() -> Element {
    rsx! {
        document::Stylesheet { href: chat::CSS }
        document::Stylesheet { href: chat_message::CSS }
    }
}
