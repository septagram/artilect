use crate::{
    actuators::chat::dto::{
        FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
        SendMessageRequest, SendMessageResponse,
    },
    service,
};

pub trait ChatClientTrait {
    async fn fetch_user_threads(
        self: &Self,
        request: service::SignedMessage<FetchUserThreadsRequest>,
    ) -> service::Result<FetchUserThreadsResponse>;

    async fn fetch_thread_messages(
        self: &Self,
        request: service::SignedMessage<FetchThreadRequest>,
    ) -> service::Result<FetchThreadResponse>;

    async fn chat(self: &Self, request: service::SignedMessage<SendMessageRequest>)
    -> service::Result<SendMessageResponse>;
}

#[cfg(feature = "chat-in")]
pub mod local;

#[cfg(feature = "chat-out")]
pub mod remote;

#[cfg(feature = "chat-in")]
pub use local::ChatClient;
#[cfg(all(feature = "chat-out", not(feature = "chat-in")))]
pub use remote::ChatClient;
