use crate::{service, Authenticated};
use artilect_macro::Authenticated;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[cfg(feature = "backend")]
#[allow(unused_imports)]
use actix::Message;

#[cfg(feature = "client")]
#[allow(unused_imports)]
use crate::Identifiable;

#[cfg(feature = "client")]
#[allow(unused_imports)]
use artilect_macro::Identifiable;

#[derive(Debug)]
#[cfg_attr(feature = "chat-in", derive(Serialize))]
#[cfg_attr(feature = "chat-out", derive(Deserialize))]
pub enum SyncUpdate<T> {
    Updated(T),
    Deleted(Uuid),
}

#[derive(Debug)]
#[cfg_attr(feature = "chat-in", derive(Serialize))]
#[cfg_attr(feature = "chat-out", derive(Deserialize))]
pub enum OneToManyChild<T> {
    Id(Uuid),
    Value(T),
}

#[derive(Debug)]
#[cfg_attr(feature = "chat-in", derive(Serialize))]
#[cfg_attr(feature = "chat-out", derive(Deserialize))]
pub struct OneToManyUpdate<T> {
    pub owner_id: Uuid,
    pub children: Vec<OneToManyChild<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "chat-in", derive(sqlx::FromRow))]
#[cfg_attr(feature = "chat-out", derive(PartialEq, Identifiable))]
pub struct User {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "chat-in", derive(sqlx::FromRow))]
#[cfg_attr(feature = "chat-out", derive(Identifiable))]
pub struct Thread {
    pub id: Uuid,
    pub name: Option<String>,
    pub owner_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "chat-out", derive(PartialEq, Identifiable))]
#[cfg_attr(feature = "chat-in", derive(sqlx::FromRow))]
pub struct ChatMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Option<Uuid>,
    pub content: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug)]
#[cfg_attr(feature = "chat-in", derive(Serialize))]
#[cfg_attr(feature = "chat-out", derive(Deserialize))]
pub struct FetchUserThreadsResponse {
    pub users: Vec<SyncUpdate<User>>,
    pub user_threads: Vec<OneToManyUpdate<Thread>>,
}

#[derive(Debug, Authenticated)]
#[cfg_attr(feature = "chat-in", derive(Message))]
#[cfg_attr(feature = "chat-in", rtype(result = "service::Result<FetchUserThreadsResponse>"))]
#[cfg_attr(feature = "chat-in", derive(Deserialize))]
#[cfg_attr(feature = "chat-out", derive(Serialize))]
pub struct FetchUserThreadsRequest {
    pub from_user_id: Uuid,
}

#[derive(Debug, Authenticated)]
#[cfg_attr(feature = "chat-in", derive(Message))]
#[cfg_attr(feature = "chat-in", rtype(result = "service::Result<FetchThreadResponse>"))]
#[cfg_attr(feature = "chat-in", derive(Deserialize))]
#[cfg_attr(feature = "chat-out", derive(Serialize))]
pub struct FetchThreadRequest {
    pub from_user_id: Uuid,
    pub thread_id: Uuid,
}

#[derive(Debug)]
#[cfg_attr(feature = "chat-in", derive(Serialize))]
#[cfg_attr(feature = "chat-out", derive(Deserialize))]
pub struct FetchThreadResponse {
    pub threads: Vec<SyncUpdate<Thread>>,
    pub thread_messages: Vec<OneToManyUpdate<ChatMessage>>,
}

#[derive(Debug, Authenticated)]
#[cfg_attr(feature = "chat-in", derive(Message))]
#[cfg_attr(feature = "chat-in", rtype(result = "service::Result<SendMessageResponse>"))]
#[cfg_attr(feature = "chat-in", derive(Deserialize))]
#[cfg_attr(feature = "chat-out", derive(Serialize))]
pub struct SendMessageRequest {
    pub from_user_id: Uuid,
    pub message: ChatMessage,
    pub is_new_thread: bool,
}

pub type SendMessageResponse = FetchThreadResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // let result = add(2, 2);
        // assert_eq!(result, 4);
    }
}
