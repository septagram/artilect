use dioxus::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

pub mod actions;
use chat_dto::{Identifiable, Message, SyncUpdate, Thread};

static USER_ID_STR: &str = dotenvy_macro::dotenv!("CHAT_USER_ID");

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub user_id: Signal<Uuid>,
    pub messages: Signal<HashMap<Uuid, SyncState<Message>>>,
    pub threads: Signal<HashMap<Uuid, SyncState<Thread>>>,
    pub thread_list: Signal<Vec<Uuid>>,
    pub thread_message_ids: Signal<HashMap<Uuid, Vec<Uuid>>>,
}

pub fn use_app_state() -> State {
    use_context_provider::<State>(|| State {
        user_id: Signal::new(Uuid::parse_str(USER_ID_STR).expect("Failed to parse CHAT_USER_ID")),
        messages: Signal::new(HashMap::new()),
        threads: Signal::new(HashMap::new()),
        thread_list: Signal::new(Vec::new()),
        thread_message_ids: Signal::new(HashMap::new()),
    })
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
    pub fn error_text(&self) -> Option<String> {
        match self {
            SyncState::Synced(_) | SyncState::Deleted => None,
            SyncState::Loading(error)
            | SyncState::Reloading(error, _)
            | SyncState::Saving(error, _)
            | SyncState::Deleting(error, _) => error.as_ref().map(|e| e.to_string()),
        }
    }

    pub fn is_synced(&self) -> bool {
        matches!(self, SyncState::Synced(_) | SyncState::Deleted)
    }

    pub fn is_syncing(&self) -> bool {
        !self.is_synced()
    }

    pub fn read(&self) -> Option<&T> {
        match self {
            SyncState::Deleted | SyncState::Loading(_) => None,
            SyncState::Synced(entity)
            | SyncState::Reloading(_, entity)
            | SyncState::Saving(_, entity)
            | SyncState::Deleting(_, entity) => Some(entity),
        }
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
