use crate::{
    precepts::vector::chat::dto::{
        FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
        SendMessageRequest, SendMessageResponse,
    },
    precept,
};

pub trait ChatClientTrait {
    async fn fetch_user_threads(
        self: &Self,
        request: precept::SignedMessage<FetchUserThreadsRequest>,
    ) -> precept::Result<FetchUserThreadsResponse>;

    async fn fetch_thread_messages(
        self: &Self,
        request: precept::SignedMessage<FetchThreadRequest>,
    ) -> precept::Result<FetchThreadResponse>;

    async fn chat(self: &Self, request: precept::SignedMessage<SendMessageRequest>)
    -> precept::Result<SendMessageResponse>;
}

#[cfg(feature = "chat-in")]
pub mod local;

#[cfg(feature = "chat-out")]
pub mod remote;

#[cfg(feature = "chat-in")]
pub use local::ChatClient;
#[cfg(all(feature = "chat-out", not(feature = "chat-in")))]
pub use remote::ChatClient;
