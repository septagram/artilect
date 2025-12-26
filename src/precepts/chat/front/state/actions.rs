use dioxus::{logger::tracing::error, prelude::*};
use futures_util::{Future, StreamExt};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{State, SyncState, consume_sync_update_batch};
use crate::{
    orchestra::AddressBook,
    precepts::chat::{
        Client,
        dto::{
            ChatMessage, FetchThreadRequest, FetchUserThreadsRequest, OneToManyChild,
            OneToManyUpdate, SendMessageRequest, SyncUpdate, Thread,
        },
    },
};

fn use_action<T, R, A>(api: Memo<A>, handler: &'static impl Fn(State, A, T) -> R) -> Coroutine<T>
where
    R: Future<Output = ()> + 'static,
    A: Clone + PartialEq,
{
    let state = use_context::<State>();
    use_coroutine(move |mut rx: UnboundedReceiver<T>| {
        let api = api.read().clone();
        async move {
            while let Some(arg) = rx.next().await {
                handler(state, api.clone(), arg).await;
            }
        }
    })
}

pub fn use_app_actions() {
    let api = use_context::<Signal<AddressBook>>();
    let chat_api = use_memo(move || api.read().chat.clone());
    use_action(chat_api, &handle_fetch_user_threads);
    use_action(chat_api, &handle_fetch_thread);
    use_action(chat_api, &handle_send_message);
}

pub type FetchUserThreadsAction = ();
async fn handle_fetch_user_threads(mut state: State, api: Client, _: FetchUserThreadsAction) {
    match api.send(FetchUserThreadsRequest {}).await {
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
async fn handle_fetch_thread(mut state: State, api: Client, thread_id: FetchThreadAction) {
    match api.send(FetchThreadRequest { thread_id }).await {
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
async fn handle_send_message(mut state: State, api: Client, action: SendMessageAction) {
    let SendMessageAction {
        thread_id,
        is_new_thread,
        content,
    } = action;
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
    let message = ChatMessage {
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
    match api
        .send(SendMessageRequest {
            message: message.clone(),
            is_new_thread,
        })
        .await
    {
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
