use dioxus::prelude::*;
use uuid::Uuid;

use crate::state::State;

pub static CSS: Asset = asset!("/src/components/chat_message.css");
#[component]
pub fn ChatMessage(message_id: Uuid) -> Element {
    let state = use_context::<State>();
    let messages = state.messages.read();
    let message_state = messages
        .get(&message_id)
        .expect("Message ID does not exist: {message_id}");

    let error_text = message_state.error_text();
    let is_syncing = message_state.is_syncing();

    match message_state.read() {
        None => rsx! {},
        Some(message) => {
            let my_user_id = *use_context::<State>().user_id.read();
            let message_source = match message.user_id {
                None => "artilect", // TODO: different style for events
                Some(id) if id == my_user_id => "me",
                Some(id) if id == Uuid::nil() => "artilect",
                _ => "other",
            };
            let class = format!("chat-message chat-message_user_{}", message_source);
            rsx! {
                div {
                    class: "{class}",
                    p {
                        class: "chat-message__text",
                        "{message.content}"
                    }
                    if is_syncing {
                        p {
                            class: "chat-message__syncing",
                            "⟳"
                        }
                    }
                    if let Some(error) = error_text {
                        p {
                            class: "chat-message__error",
                            "Error: {error}"
                        }
                    }
                }
            }
        }
    }
}
