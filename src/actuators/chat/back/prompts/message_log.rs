use once_cell::sync::Lazy;
use time::format_description::{self, FormatItem};
use uuid::Uuid;

use crate::infer;
use super::super::super::dto::User;

static DATE_FORMAT: Lazy<Vec<FormatItem>> = Lazy::new(|| {
    format_description::parse("[weekday] [year]-[month]-[day]")
        .expect("Failed to parse date format")
});

static TIME_FORMAT_LONG: Lazy<Vec<FormatItem>> = Lazy::new(|| {
    format_description::parse("[hour]:[minute]:[second]")
        .expect("Failed to parse long time format")
});

static TIME_FORMAT_SHORT: Lazy<Vec<FormatItem>> = Lazy::new(|| {
    format_description::parse("[hour]:[minute]")
        .expect("Failed to parse short time format")
});

#[derive(sqlx::FromRow)]
pub struct MessageLogItemRow {
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub content: String,
    pub created_at: time::OffsetDateTime,
}

pub struct MessageLogItem {
    pub user: Option<User>,
    pub content: String,
    pub created_at: time::OffsetDateTime,
}

impl From<MessageLogItemRow> for MessageLogItem {
    fn from(row: MessageLogItemRow) -> Self {
        Self {
            user: match (row.user_id, row.user_name) {
                (Some(id), Some(name)) => Some(User { id, name }),
                _ => None
            },
            content: row.content,
            created_at: row.created_at,
        }
    }
}

impl MessageLogItem {
    pub fn is_event(&self) -> bool {
        self.user.is_none()
    }

    pub fn is_own_message(&self) -> bool {
        self.user.as_ref().map(|u| u.id == Uuid::nil()).unwrap_or(false)
    }
}

pub fn message_log(messages: Vec<MessageLogItem>) -> Result<impl Iterator<Item = infer::Message>, time::error::Format> {
    let now = time::OffsetDateTime::now_utc();
    let mut last_date = None;
    let mut output_messages = Vec::new();

    for message in messages.into_iter().rev() {
        let date = message.created_at.date();
        let do_show_date = match last_date {
            None => true,
            Some(last_date) => last_date != date,
        };
        if do_show_date {
            last_date = Some(date);
        }
        let date_attr = if do_show_date {
            Some(date.format(&DATE_FORMAT)?)
        } else {
            None
        };

        let message_time = message.created_at.to_offset(time::UtcOffset::UTC);
        let elapsed = now - message_time;
        let time_attr = if elapsed.whole_minutes() < 1 {
            message.created_at.time().format(&TIME_FORMAT_LONG)?
        } else {
            message.created_at.time().format(&TIME_FORMAT_SHORT)?
        };

        let role = if message.is_own_message() {
            infer::MessageRole::Assistant
        } else {
            infer::MessageRole::User
        };

        match message.user {
            None => output_messages.push(infer::Message::new_text_system(markup::new! {
                event [date = &date_attr, time = &time_attr] {
                    @message.content
                }
            }.to_string())),
            Some(user) => {
                let context = markup::new! {
                    nextMessageInfo [date = &date_attr, time = &time_attr, from = &user.name];
                };
                output_messages.push(infer::Message::new_text_system(context.to_string()));
                output_messages.push(infer::Message::new_text(role, message.content));
            }
        };
    }

    Ok(output_messages.into_iter())
}
