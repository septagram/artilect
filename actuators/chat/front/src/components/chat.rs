use dioxus::prelude::*;
use uuid::Uuid;

use super::ChatMessage;
use crate::state::{
    actions::{FetchThreadAction, SendMessageAction},
    State,
};

pub static CSS: Asset = asset!("/src/components/chat.css");

#[component]
pub fn Chat(thread_id: Option<Uuid>) -> Element {
    let state = use_context::<State>();
    let mut input = use_signal(|| String::new());
    let navigator = use_navigator();
    let dispatch_send_message = use_coroutine_handle::<SendMessageAction>();
    let dispatch_fetch_thread = use_coroutine_handle::<FetchThreadAction>();

    use_effect(use_reactive!(|thread_id| {
        if let Some(thread_id) = thread_id {
            dispatch_fetch_thread.send(thread_id);
        }
    }));

    let mut handle_send = move || {
        let (is_new_thread, thread_id) = match thread_id {
            Some(thread_id) => (false, thread_id),
            None => (true, Uuid::new_v4()),
        };
        dispatch_send_message.send(SendMessageAction {
            content: input.read().clone(),
            thread_id,
            is_new_thread,
        });
        input.set(String::new());
        if is_new_thread {
            navigator.push(format!("/chat/{}", thread_id));
        }
    };

    let handle_keypress = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            handle_send();
        }
    };

    let thread_message_ids = match thread_id {
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
                for message_id in thread_message_ids.iter().rev() {
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
