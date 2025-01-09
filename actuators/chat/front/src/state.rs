use dioxus::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use chat_dto::{Identifiable, Message, SyncUpdate, Thread};

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub user_id: Signal<Uuid>,
    pub messages: Signal<HashMap<Uuid, SyncState<Message>>>,
    pub threads: Signal<HashMap<Uuid, SyncState<Thread>>>,
    pub thread_id: Signal<Option<Uuid>>,
    pub thread_message_ids: Signal<HashMap<Uuid, Vec<Uuid>>>,
}

#[derive(Debug)]
pub struct SyncError {
    message: String,
    cause: Option<Box<dyn std::error::Error>>,
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_ref().map(|e| e.as_ref())
    }
}

impl SyncError {
    pub fn new<S: Into<String>>(message: S) -> Self {
        Self {
            message: message.into(),
            cause: None,
        }
    }

    pub fn with_cause<S, E>(message: S, cause: E) -> Self
    where
        S: Into<String>,
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            message: message.into(),
            cause: Some(Box::new(cause)),
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Debug)]
pub enum SyncState<T: Identifiable> {
    // In-progress sync
    Loading(Option<SyncError>),
    Reloading(Option<SyncError>, T),
    Saving(Option<SyncError>, T),
    Deleting(Option<SyncError>, T),

    // Synced state
    Synced(T),
    Deleted,
}

impl<T: Identifiable> SyncState<T> {
    pub fn is_synced(&self) -> bool {
        matches!(self, SyncState::Synced(_) | SyncState::Deleted)
    }
}

pub fn consume_sync_update_batch<T: Identifiable>(
    map: &mut HashMap<Uuid, SyncState<T>>,
    updates: Option<Vec<SyncUpdate<T>>>,
) {
    if let Some(updates) = updates {
        for update in updates {
            let id = match &update {
                SyncUpdate::Updated(entity) => entity.get_id(),
                SyncUpdate::Deleted(id) => *id,
            };
            let state = match update {
                SyncUpdate::Updated(entity) => SyncState::Synced(entity),
                SyncUpdate::Deleted(_) => SyncState::Deleted,
            };
            map.insert(id, state);
        }
    }
}
