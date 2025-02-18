use dioxus::logger::tracing::error;
use dioxus::prelude::*;
use futures_util::{Future, StreamExt};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{consume_sync_update_batch, State, SyncState};
use crate::api;
use chat_dto::{Message, OneToManyChild, OneToManyUpdate, SyncUpdate, Thread};

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
    use_action::<FetchUserThreadsAction, _>(&handle_fetch_user_threads);
    use_action::<FetchThreadAction, _>(&handle_fetch_thread);
    use_action::<SendMessageAction, _>(&handle_send_message);
}

pub type FetchUserThreadsAction = ();
async fn handle_fetch_user_threads(mut state: State, _: FetchUserThreadsAction) {
    match api::fetch_user_threads().await {
        Ok(response) => {
            let mut thread_updates = Vec::new();
            state.thread_list.with_mut(|thread_list| {
                thread_list.clear();
                for OneToManyUpdate { children, .. } in response.user_threads {
                    for child in children {
                        match child {
                            OneToManyChild::Value(thread) => {
                                thread_list.push(thread.id);
                                thread_updates.push(SyncUpdate::Updated(thread));
                            }
                            _ => {}
                        }
                    }
                }
            });
            state.threads.with_mut(|threads_state| {
                consume_sync_update_batch(threads_state, Some(thread_updates));
            });
        }
        Err(error) => {
            error!("Error fetching user threads: {}", error);
        }
    }
}

pub type FetchThreadAction = Uuid;
async fn handle_fetch_thread(mut state: State, thread_id: FetchThreadAction) {
    match api::fetch_thread(thread_id).await {
        Ok(response) => {
            state.threads.with_mut(|t| {
                consume_sync_update_batch(t, Some(response.threads));
            });
            // Make consume_one_to_many_update_batch fn!
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
            error!("Error fetching thread {thread_id}: {}", error);
        }
    }
}

pub struct SendMessageAction {
    pub thread_id: Uuid,
    pub is_new_thread: bool,
    pub content: String,
}
async fn handle_send_message(mut state: State, action: SendMessageAction) {
    let SendMessageAction { thread_id, is_new_thread, content } = action;
    if is_new_thread {
        let now = OffsetDateTime::now_utc();
        state.threads.with_mut(|t| {
            t.insert(
                thread_id,
                SyncState::Saving(
                    None,
                    Thread {
                        id: thread_id,
                        name: None,
                        owner_id: *state.user_id.read(),
                        created_at: now,
                        updated_at: now,
                    },
                ),
            )
        });
        state.thread_list.with_mut(|thread_list| {
            thread_list.insert(0, thread_id);
        });
    };
    let now = OffsetDateTime::now_utc();
    let message = Message {
        id: Uuid::new_v4(),
        thread_id,
        user_id: Some(*state.user_id.read()),
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
            // Make consume_one_to_many_update_batch fn!
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
