#![feature(let_chains)]

use dioxus::logger::tracing::{error, info, Level};
use dioxus::prelude::*;
use futures_util::StreamExt;
use reqwest::Client;
use std::collections::HashMap;
use std::error::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use chat_dto::{Message, OneToManyChild, SendMessageRequest, SendMessageResponse, Thread};

mod state;
use state::{consume_sync_update_batch, SyncState};

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
    pub messages: Signal<HashMap<Uuid, SyncState<Message>>>,
    pub threads: Signal<HashMap<Uuid, SyncState<Thread>>>,
    pub thread_id: Signal<Option<Uuid>>,
    pub thread_message_ids: Signal<HashMap<Uuid, Vec<Uuid>>>,
}

#[component]
fn Home() -> Element {
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
            match send_message(&message, is_new_thread).await {
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
        style { {include_str!("style.css")} }
    }
}

#[component]
fn ChatMessage(message_id: Uuid) -> Element {
    let state = use_context::<State>();
    let messages = state.messages.read();
    let message_state = messages
        .get(&message_id)
        .expect("Message ID does not exist: {message_id}");

    let error_text: Option<String> = match &message_state {
        SyncState::Synced(_) | SyncState::Deleted => None,
        SyncState::Loading(error)
        | SyncState::Reloading(error, _)
        | SyncState::Saving(error, _)
        | SyncState::Deleting(error, _) => error.as_ref().map(|e| e.to_string()),
    };

    let is_syncing = !message_state.is_synced();

    match message_state {
        SyncState::Deleted | SyncState::Loading(_) => rsx! {},
        SyncState::Synced(message)
        | SyncState::Reloading(_, message)
        | SyncState::Saving(_, message)
        | SyncState::Deleting(_, message) => {
            let message = message.clone();
            let my_user_id = *use_context::<State>().user_id.read();
            let message_source = match message.user_id {
                id if id == my_user_id => "my",
                id if id == Uuid::nil() => "artilect",
                _ => "other",
            };
            let class = format!("chat__message chat__message--{}", message_source);
            rsx! {
                div {
                    class: "{class}",
                    p {
                        class: "chat__message-text",
                        "{message.content}"
                    }
                    if is_syncing {
                        p {
                            class: "chat__message-syncing",
                            "⟳"
                        }
                    }
                    if let Some(error) = error_text {
                        p {
                            class: "chat__message-error",
                            "Error: {error}"
                        }
                    }
                }
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
