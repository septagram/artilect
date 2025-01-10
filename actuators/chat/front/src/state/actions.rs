use dioxus::prelude::*;
use dioxus::logger::tracing::error;
use futures_util::{Future, StreamExt};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{consume_sync_update_batch, State, SyncState};
use crate::api;
use chat_dto::{Message, OneToManyChild, Thread};

fn use_action<T, F>(handler: &'static impl Fn(State, T) -> F) -> Coroutine<T>
where
    F: Future<Output = ()> + 'static,
{
    let state = use_context::<State>();
    use_coroutine(move |mut rx: UnboundedReceiver<T>| async move {
        while let Some(arg) = rx.next().await {
            handler(state, arg).await;
        }
    })
}

pub fn use_app_actions() {
    use_action::<SendMessageAction, _>(&handle_send_message);
}

pub type SendMessageAction = String;
async fn handle_send_message(mut state: State, content: SendMessageAction) {
    let current_thread_id = *state.thread_id.read();
    let is_new_thread = current_thread_id.is_none();
    let thread_id = current_thread_id.unwrap_or_else(|| {
        let id = Uuid::new_v4();
        let now = OffsetDateTime::now_utc();
        state.threads.with_mut(|t| {
            t.insert(
                id,
                SyncState::Saving(
                    None,
                    Thread {
                        id,
                        name: None,
                        owner_id: *state.user_id.read(),
                        created_at: now,
                        updated_at: now,
                    },
                ),
            )
        });
        state.thread_id.set(Some(id));
        id
    });
    let now = OffsetDateTime::now_utc();
    let message = Message {
        id: Uuid::new_v4(),
        thread_id,
        user_id: *state.user_id.read(),
        content,
        created_at: now,
        updated_at: None,
    };
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
