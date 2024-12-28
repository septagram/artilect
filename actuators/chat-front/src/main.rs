#![feature(let_chains)]

use dioxus::prelude::*;
use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error;
use futures_util::StreamExt;
use dioxus::logger::tracing::{Level, trace, debug, info, warn, error};

static BASE_URL: &str = "http://localhost:3001";

async fn send_message(message: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();
    match client
        .post(format!("{BASE_URL}/chat"))
        .json(&json!({"message": message}))
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(json) = res.json::<Value>().await
                && json.get("role").and_then(Value::as_str) == Some("assistant")
                && let Some(content) = json.get("content").and_then(Value::as_str)
            {
                Ok(String::from(content))
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
    dioxus_logger::init(Level::INFO);
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

#[derive(Clone, Debug)]
struct Message {
    text: String,
    is_user: bool,
}

#[component]
fn Home() -> Element {
    let mut input = use_signal(|| String::new());
    let mut messages = use_signal(|| initial_messages());
    info!("Test console, message count: {}", messages.len());

    let handle_send = use_coroutine(move |mut rx: UnboundedReceiver<String> | async move {
        while let Some(message) = rx.next().await {
            match send_message(&message).await {
                Ok(response) => {
                    let mut new_messages = messages.read().clone();
                    new_messages.push(Message {
                        text: message,
                        is_user: true,
                    });
                    new_messages.push(Message {
                        text: response,
                        is_user: false,
                    });
                    info!("Updated messages: {:?}", new_messages);
                    messages.set(new_messages);
                }
                Err(error) => {
                    web_sys::window().unwrap().alert_with_message(error.to_string().as_str()).unwrap();
                }
            }
        }
    });

    let handle_keypress = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            handle_send.send(input.read().clone());
            input.set(String::new());
        }
    };

    rsx! {
        div { class: "chat",
            div { class: "chat__history",
                for (index, msg) in messages.iter().enumerate() {
                    div {
                        key: "{index}",
                        class: if msg.is_user { "chat__message chat__message--user" } else { "chat__message chat__message--system" },
                        p { class: "chat__message-text", "{msg.text}" }
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
                    onclick: move |_| handle_send.send(input.read().clone()),
                    "Send"
                }
            }
        }
        style { {include_str!("style.css")} }
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
