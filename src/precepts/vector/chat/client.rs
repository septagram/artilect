use artilect_macro::{if_precept_in, if_precept_out};
use crate::{
    precepts::vector::chat::dto::{
        FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
        SendMessageRequest, SendMessageResponse,
    },
    precept,
};

pub trait ChatClientTrait {
    fn fetch_user_threads(
        self: &Self,
        request: FetchUserThreadsRequest,
    ) -> impl std::future::Future<Output = precept::Result<FetchUserThreadsResponse>>;

    fn fetch_thread_messages(
        self: &Self,
        request: FetchThreadRequest,
    ) -> impl std::future::Future<Output = precept::Result<FetchThreadResponse>>;

    fn chat(self: &Self, request: SendMessageRequest)
    -> impl std::future::Future<Output = precept::Result<SendMessageResponse>>;
}

#[if_precept_in(chat)]
pub mod local;

#[if_precept_out(chat)]
pub mod remote;

#[if_precept_in(chat)]
pub use local::ChatClient;
#[cfg(all(feature = "chat-out", not(feature = "chat-in")))]
pub use remote::ChatClient;

#[if_precept_in(chat)]
pub use local::GlobalChatClient;
#[cfg(all(feature = "chat-out", not(feature = "chat-in")))]
pub use remote::GlobalChatClient;
