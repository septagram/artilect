use dioxus::prelude::*;

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

#[derive(Clone)]
struct Message {
    text: String,
    is_user: bool,
}

fn send_message(message: &str) {
    web_sys::window().unwrap().alert_with_message(message).unwrap();
}

#[component]
fn Home() -> Element {
    let mut input = use_signal(|| String::new());

    let handle_send = move |_| {
        if !input.read().is_empty() {
            send_message(&input.read());
            // do_alert(&input.read()());
            //document::window().alert(&format!("Message sent: {}", input));
            input.set(String::new());
        }
    };

    let handle_keypress = move |evt: KeyboardEvent| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            send_message(&input.read());
            input.set(String::new());
        }
    };

    rsx! {
        div { class: "chat",
            div { class: "chat__history",
                for msg in initial_messages() {
                    div {
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
                    onclick: handle_send,
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
