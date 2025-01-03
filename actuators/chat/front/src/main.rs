#![feature(let_chains)]

use chat_dto::{Message, SendMessageRequest, SendMessageResponse, Thread};
use dioxus::logger::tracing::{debug, error, info, trace, warn, Level};
use dioxus::prelude::*;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use time::OffsetDateTime;
use uuid::Uuid;

static BASE_URL: &str = dotenvy_macro::dotenv!("CHAT_BASE_URL");
static USER_ID_STR: &str = dotenvy_macro::dotenv!("CHAT_USER_ID");

async fn send_message(
    message: &Message,
    is_new_thread: bool,
) -> Result<chat_dto::SendMessageResponse, Box<dyn Error>> {
    let client = Client::new();
    match client
        .post(format!("{BASE_URL}/chat"))
        .json(&SendMessageRequest {
            message: message.clone(),
            is_new_thread,
        })
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(response) = res.json::<SendMessageResponse>().await {
                Ok(response)
            } else {
                Err("Failed to send message".into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    // #[route("/blog/:id")]
    // Blog { id: i32 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
fn initial_messages() -> Vec<Message> {
    vec![]
}

fn main() {
    dioxus_logger::init(Level::INFO).unwrap();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        // document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[derive(Debug, Clone, Copy)]
struct State {
    pub user_id: Signal<Uuid>,
    pub unsynced_ids: Signal<HashSet<Uuid>>,
    pub messages: Signal<HashMap<Uuid, Message>>,
    pub threads: Signal<HashMap<Uuid, Thread>>,
    pub thread_id: Signal<Option<Uuid>>,
    pub thread_message_ids: Signal<Vec<Uuid>>,
}

#[component]
fn Home() -> Element {
    let mut state = use_context_provider::<State>(|| State {
        user_id: Signal::new(Uuid::parse_str(USER_ID_STR).expect("Failed to parse CHAT_USER_ID")),
        unsynced_ids: Signal::new(HashSet::new()),
        messages: Signal::new(HashMap::new()),
        threads: Signal::new(HashMap::new()),
        thread_id: Signal::new(None),
        thread_message_ids: Signal::new(initial_messages().iter().map(|m| m.id).collect()),
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
                    .with_mut(|t| t.insert(id, Thread { id, name: None }));
                state.unsynced_ids.with_mut(|ids| ids.insert(id));
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
            state.unsynced_ids.with_mut(|ids| {
                ids.insert(message.id);
            });
            state.messages.with_mut(|m| {
                m.insert(message.id, message.clone());
            });
            state
                .thread_message_ids
                .with_mut(|ids| ids.push(message.id));
            match send_message(&message, is_new_thread).await {
                Ok(response) => {
                    let old_thread_id = state.thread_id.read().unwrap();
                    let thread = response.threads.get(&old_thread_id);
                    if let Some(thread) = thread {
                        if thread.id != old_thread_id {
                            state.threads.with_mut(|t| {
                                t.remove(&old_thread_id);
                            });
                            state.thread_id.set(Some(thread.id));
                        }
                        state.threads.with_mut(|t| {
                            t.insert(thread.id, thread.clone());
                        });
                    };
                    state.messages.with_mut(|m| {
                        for (message_id, message) in response.messages {
                            if message.id != message_id {
                                state.thread_message_ids.with_mut(|ids| {
                                    for id in ids.iter_mut() {
                                        if *id == message_id {
                                            *id = message.id;
                                            break;
                                        }
                                    }
                                });
                                m.remove(&message_id);
                            } else {
                                state.thread_message_ids.with_mut(|ids| {
                                    ids.push(message.id);
                                });
                            }
                            m.insert(message.id, message);
                        }
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
    rsx! {
        div { class: "chat",
            div { class: "chat__history",
                for message_id in state.thread_message_ids.read().clone() {
                    ChatMessage {
                        key: "{message_id}",
                        message_id,
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
        style { {include_str!("style.css")} }
    }
}

#[component]
fn ChatMessage(message_id: Uuid) -> Element {
    let message = use_context::<State>()
        .messages
        .read()
        .get(&message_id)
        .cloned()
        .expect("Message ID does not exist");
    let is_user_message = message.user_id == *use_context::<State>().user_id.read();
    let class = match is_user_message {
        true => "chat__message chat__message--user",
        false => "chat__message chat__message--system",
    };

    rsx! {
        div {
            class: "{class}",
            p {
                class: "chat__message-text",
                "{message.content}"
            }
        }
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
            }
            // Link {
            //     to: Route::Blog { id: 1 },
            //     "Blog"
            // }
        }

        Outlet::<Route> {}
    }
}
