use dioxus_lib::prelude::*;

#[derive(Clone, PartialEq, sqlx::FromRow)]
pub struct MessageLogItem {
    pub user_name: String,
    pub content: String,
    pub created_at: time::OffsetDateTime,
}

#[allow(non_snake_case, non_upper_case_globals)]
pub mod dioxus_elements {
    use super::*;

    crate::builder_constructors! {
        messageLog None {
            title: String DEFAULT,
        };
        message None {
            date: String DEFAULT,
            time: String DEFAULT,
            from: String DEFAULT,
        };
    }

    pub mod elements {
        pub use super::*;
    }
}

#[component]
pub fn MessageLog(thread_name: Option<String>, messages: Vec<MessageLogItem>) -> Element {
    rsx! {
        messageLog {
            title: thread_name,
            for message in messages.into_iter().rev() {
                message {
                    date: message.created_at.date().to_string(),
                    time: message.created_at.time().to_string(),
                    from: message.user_name,
                    {message.content}
                }
            }
        }
    }
}
