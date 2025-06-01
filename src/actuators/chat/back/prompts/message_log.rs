use once_cell::sync::Lazy;
use time::format_description::{self, FormatItem};
use uuid::Uuid;

use crate::infer;

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

#[derive(Clone, PartialEq, sqlx::FromRow)]
pub struct MessageLogItem {
    pub user_id: Option<Uuid>,
    pub user_name: String,
    pub content: String,
    pub created_at: time::OffsetDateTime,
}

impl MessageLogItem {
    pub fn is_event(&self) -> bool {
        self.user_id.is_none()
    }

    pub fn is_own_message(&self) -> bool {
        self.user_id == Some(Uuid::nil())
    }
}

pub fn message_log(messages: Vec<MessageLogItem>) -> Result<impl Iterator<Item = infer::Message>, time::error::Format> {
    let now = time::OffsetDateTime::now_utc();
    let mut last_date = None;

    Ok(messages.into_iter().rev().map(move |message| -> Result<infer::Message, time::error::Format> {
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

        let content = if message.is_event() {
            markup::new! {
                event [date = &date_attr, time = &time_attr] {
                    @message.content
                }
            }.to_string().into_boxed_str()
        } else {
            format!("{}\n{}", markup::new! {
                context {
                    messageInfo [date = &date_attr, time = &time_attr, from = &message.user_name];
                }
            }, message.content).into_boxed_str()
        };

        Ok(infer::Message { role, content })
    }).collect::<Result<Vec<_>, _>>()?.into_iter())
}
