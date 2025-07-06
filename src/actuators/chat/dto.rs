use artilect_macro::dto;
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

// cargo expand --lib actuators::chat::dto --features="server-http2 chat-in auth-out" 2> /dev/null | head -n 100
#[dto(chat, response)]
pub enum SyncUpdate<T> {
    Updated(T),
    Deleted(Uuid),
}

#[dto(chat, response)]
pub enum OneToManyChild<T> {
    Id(Uuid),
    Value(T),
}

#[dto(chat, response)]
pub struct OneToManyUpdate<T> {
    pub owner_id: Uuid,
    pub children: Vec<OneToManyChild<T>>,
}

#[dto(chat, db, ui, clone, request, response)]
pub struct User {
    pub id: Uuid,
    pub name: String,
}

#[dto(chat, db, ui, clone, request, response)]
pub struct Thread {
    pub id: Uuid,
    pub name: Option<String>,
    pub owner_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[dto(chat, db, ui, clone, request, response)]
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

#[dto(chat, response)]
pub struct FetchUserThreadsResponse {
    pub users: Vec<SyncUpdate<User>>,
    pub user_threads: Vec<OneToManyUpdate<Thread>>,
}

#[dto(chat, request)]
#[actix_message(FetchUserThreadsResponse, FetchUserThreadsMessage)]
pub struct FetchUserThreadsRequest {
    pub from_user_id: Uuid,
}

#[dto(chat, request)]
#[actix_message(FetchThreadResponse, FetchThreadMessage)]
pub struct FetchThreadRequest {
    pub from_user_id: Uuid,
    pub thread_id: Uuid,
}

#[dto(chat, response)]
pub struct FetchThreadResponse {
    pub threads: Vec<SyncUpdate<Thread>>,
    pub thread_messages: Vec<OneToManyUpdate<ChatMessage>>,
}

#[dto(chat, request)]
#[actix_message(SendMessageResponse, SendMessageMessage)]
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
