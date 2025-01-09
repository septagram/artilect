use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[cfg(feature = "frontend")]
use chat_macros::Identifiable;

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}

#[derive(Debug)]
#[cfg_attr(feature = "backend", derive(Serialize))]
#[cfg_attr(feature = "frontend", derive(Deserialize))]
pub enum SyncUpdate<T> {
    Updated(T),
    Deleted(Uuid),
}

#[derive(Debug)]
#[cfg_attr(feature = "backend", derive(Serialize))]
#[cfg_attr(feature = "frontend", derive(Deserialize))]
pub enum OneToManyChild<T> {
    Id(Uuid),
    Value(T),
}

#[derive(Debug)]
#[cfg_attr(feature = "backend", derive(Serialize))]
#[cfg_attr(feature = "frontend", derive(Deserialize))]
pub struct OneToManyUpdate<T> {
    pub owner_id: Uuid,
    pub children: Vec<OneToManyChild<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "backend", derive(sqlx::FromRow))]
#[cfg_attr(feature = "frontend", derive(PartialEq, Identifiable))]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "backend", derive(sqlx::FromRow))]
#[cfg_attr(feature = "frontend", derive(PartialEq, Identifiable))]
pub struct Thread {
    pub id: Uuid,
    pub name: Option<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "backend", derive(Deserialize))]
#[cfg_attr(feature = "frontend", derive(Serialize))]
pub struct SendMessageRequest {
    pub message: Message,
    pub is_new_thread: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "backend", derive(Serialize))]
#[cfg_attr(feature = "frontend", derive(Deserialize))]
pub struct SendMessageResponse {
    pub threads: Vec<SyncUpdate<Thread>>,
    pub thread_messages: Vec<OneToManyUpdate<Message>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "frontend", derive(PartialEq, Identifiable))]
#[cfg_attr(feature = "backend", derive(sqlx::FromRow))]
pub struct Message {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub created_at: OffsetDateTime,
    pub updated_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // let result = add(2, 2);
        // assert_eq!(result, 4);
    }
}
