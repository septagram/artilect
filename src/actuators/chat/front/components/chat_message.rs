use dioxus::prelude::*;
use uuid::Uuid;

use crate::actuators::chat::front::state::State;

pub static CSS: Asset = asset!("/src/actuators/chat/front/components/chat_message.css");
#[component]
pub fn ChatMessage(message_id: Uuid) -> Element {
    let b = classnames::classname("chat-message");
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
            let b = match message.user_id {
                None => b.attr("event"),
                Some(id) => {
                    let message_source = match id {
                        _ if id == my_user_id => "user-me",
                        _ if id == Uuid::nil() => "user-artilect",
                        _ => "user-other",
                    };
                    b.attr("message").attr(message_source)
                }
            };
            let rendered_markdown = match markdown::to_html_with_options(&message.content, &markdown::Options::gfm()) {
                Ok(rendered) => rendered,
                Err(_) => markdown::to_html(&message.content),
            };
            rsx! {
                div {
                    class: b.to_string(),
                    div {
                        class: b.el("text").to_string(),
                        dangerous_inner_html: rendered_markdown
                    }
                    if is_syncing {
                        p {
                            class: b.el("syncing").to_string(),
                            "⟳"
                        }
                    }
                    if let Some(error) = error_text {
                        p {
                            class: b.el("error").to_string(),
                            "Error: {error}"
                        }
                    }
                }
            }
        }
    }
}
