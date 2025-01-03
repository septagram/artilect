#![feature(let_chains)]

use dioxus::prelude::*;
use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error;
use futures_util::StreamExt;
use dioxus::logger::tracing::{Level, trace, debug, info, warn, error};
use uuid::Uuid;

use chat_dto::SendMessageResponse;

static BASE_URL: &str = dotenvy_macro::dotenv!("CHAT_BASE_URL");
#[derive(Debug, Clone, PartialEq)]
pub struct UnsyncedMessage {
    pub thread_id: Option<Uuid>, // Optional since it may be a new thread
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Synced(chat_dto::Message),
    Unsynced(UnsyncedMessage),
}

async fn send_message(message: &str) -> Result<chat_dto::Message, Box<dyn Error>> {
    let client = Client::new();
    match client
        .post(format!("{BASE_URL}/chat"))
        .json(&json!({"message": message}))
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(response) = res.json::<SendMessageResponse>().await {
                Ok(response.message)
            } else {
                Err("Failed to send message".into())
            }
        },
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

#[component]
fn Home() -> Element {
    let mut input = use_signal(|| String::new());
    let mut messages = use_signal(|| initial_messages());
    info!("Test console, message count: {}", messages.len());

    let handle_send = use_coroutine(move |mut rx: UnboundedReceiver<()> | async move {
        while let Some(_) = rx.next().await {
            let message = input.read().clone();
            input.set(String::new());
            let mut messages_with_sent = messages.read().clone();
            messages_with_sent.push(Message::Unsynced(UnsyncedMessage {
                thread_id: None,
                content: message.clone(),
            }));
            messages.set(messages_with_sent.clone());
            match send_message(&message).await {
                Ok(response) => {
                    messages_with_sent.push(Message::Synced(response));
                    info!("Updated messages: {:?}", messages_with_sent);
                    messages.set(messages_with_sent);
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
                for (index, msg) in messages.iter().enumerate() {
                    ChatMessage {
                        key: "{index}",
                        message: msg.clone()
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
fn ChatMessage(message: Message) -> Element {
    let class = match message {
        Message::Unsynced(_) => "chat__message chat__message--user",
        Message::Synced(ref m) => if m.user_id.is_nil() {
            "chat__message chat__message--system"
        } else {
            "chat__message chat__message--user"
        }
    };

    let content = match message {
        Message::Unsynced(m) => m.content,
        Message::Synced(m) => m.content
    };

    rsx! {
        div {
            class: "{class}",
            p { 
                class: "chat__message-text",
                "{content}"
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
