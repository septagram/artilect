use dioxus::logger::tracing::error;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

use super::ChatMessage;
use crate::api;
use crate::state::{consume_sync_update_batch, State, SyncState};
use chat_dto::{Message, OneToManyChild, Thread};

static USER_ID_STR: &str = dotenvy_macro::dotenv!("CHAT_USER_ID");

pub static CSS: Asset = asset!("/src/components/chat.css");

#[component]
pub fn Chat() -> Element {
    let mut state = use_context_provider::<State>(|| State {
        user_id: Signal::new(Uuid::parse_str(USER_ID_STR).expect("Failed to parse CHAT_USER_ID")),
        messages: Signal::new(HashMap::new()),
        threads: Signal::new(HashMap::new()),
        thread_id: Signal::new(None),
        thread_message_ids: Signal::new(HashMap::new()),
    });
    let mut input = use_signal(|| String::new());

    let handle_send = use_coroutine(move |mut rx: UnboundedReceiver<()>| async move {
        while let Some(_) = rx.next().await {
            let current_thread_id = *state.thread_id.read();
            let is_new_thread = current_thread_id.is_none();
            let thread_id = current_thread_id.unwrap_or_else(|| {
                let id = Uuid::new_v4();
                state
                    .threads
                    .with_mut(|t| t.insert(id, SyncState::Saving(None, Thread { id, name: None })));
                state.thread_id.set(Some(id));
                id
            });
            let message = Message {
                id: Uuid::new_v4(),
                thread_id: thread_id,
                user_id: *state.user_id.read(),
                content: input.read().clone(),
                created_at: OffsetDateTime::now_utc(),
                updated_at: None,
            };
            input.set(String::new());
            state.messages.with_mut(|m| {
                m.insert(message.id, SyncState::Saving(None, message.clone()));
            });
            state
                .thread_message_ids
                .with_mut(|ids| ids.entry(thread_id).or_insert(vec![]).push(message.id));
            match api::send_message(&message, is_new_thread).await {
                Ok(response) => {
                    state.threads.with_mut(|t| {
                        consume_sync_update_batch(t, Some(response.threads));
                    });
                    state.messages.with_mut(|messages| {
                        state.thread_message_ids.with_mut(|thread_message_ids| {
                            for update in response.thread_messages {
                                let mut cur_thread_message_ids = vec![]; //thread_message_ids.entry(update.owner_id).or_insert(vec![]);
                                for child in update.children {
                                    let id = match child {
                                        OneToManyChild::Id(id) => id,
                                        OneToManyChild::Value(child) => {
                                            let id = child.id;
                                            messages.insert(id, SyncState::Synced(child));
                                            id
                                        }
                                    };
                                    cur_thread_message_ids.push(id);
                                }
                                thread_message_ids.insert(update.owner_id, cur_thread_message_ids);
                            }
                        });
                    });
                }
                Err(error) => {
                    error!("Error sending message: {}", error);
                }
            }
        }
    });

    let handle_keypress = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            handle_send.send(());
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
                    onclick: move |_| handle_send.send(()),
                    "Send"
                }
            }
        }
    }
}
