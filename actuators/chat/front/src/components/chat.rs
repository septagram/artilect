use dioxus::prelude::*;
use uuid::Uuid;

use super::ChatMessage;
use crate::state::{State, actions::SendMessageAction};

pub static CSS: Asset = asset!("/src/components/chat.css");

#[component]
pub fn Chat(thread_id: Option<Uuid>) -> Element {
    let mut state = use_context::<State>();
    state.thread_id.set(thread_id);
    let mut input = use_signal(|| String::new());
    let dispatch_send_message = use_coroutine_handle::<SendMessageAction>();

    let mut handle_send = move || {
        dispatch_send_message.send(input.read().clone());
        input.set(String::new());
    };

    let handle_keypress = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            handle_send();
        }
    };

    let thread_message_ids = match *state.thread_id.read() {
        Some(thread_id) => state
            .thread_message_ids
            .read()
            .get(&thread_id)
            .unwrap_or(&vec![])
            .clone(),
        None => vec![],
    };

    rsx! {
        div { class: "chat",
            div { class: "chat__history",
                for message_id in &thread_message_ids {
                    ChatMessage {
                        key: "{message_id}",
                        message_id: *message_id,
                    }
                }
            }
            div { class: "chat__input",
                textarea {
                    class: "chat__input-field",
                    placeholder: "Type your message...",
                    value: "{input}",
                    onkeydown: handle_keypress,
                    oninput: move |evt| input.set(evt.value().clone()),
                }
                button {
                    class: "chat__send-button",
                    disabled: input.read().is_empty(),
                    onclick: move |_| handle_send(),
                    "Send"
                }
            }
        }
    }
}

#[component]
pub fn NewChat() -> Element {
    rsx! { Chat { thread_id: None } }
}
