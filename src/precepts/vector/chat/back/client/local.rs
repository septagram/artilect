use std::sync::Arc;

use actix::Addr;

use crate::{
    precepts::vector::chat::{
        back::actor::ChatPrecept,
        dto::{
            FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
            SendMessageRequest, SendMessageResponse,
        }
    },
    precept,
    precept::ActixResult,
};
use crate::precept::SignedMessage;

#[derive(Clone)]
pub struct ChatClient {
    actor: Arc<Addr<ChatPrecept>>,
}

impl ChatClient {
    pub fn new(addr: Arc<Addr<ChatPrecept>>) -> Self {
        Self { actor: addr }
    }
}

impl super::ChatClientTrait for ChatClient {
    async fn fetch_user_threads(self: &Self, request: SignedMessage<FetchUserThreadsRequest>) -> precept::Result<FetchUserThreadsResponse> {
        self.actor.send(request).await.into_precept_result()
    }

    async fn fetch_thread_messages(self: &Self, request: SignedMessage<FetchThreadRequest>) -> precept::Result<FetchThreadResponse> {
        self.actor.send(request).await.into_precept_result()
    }

    async fn chat(self: &Self, request: SignedMessage<SendMessageRequest>) -> precept::Result<SendMessageResponse> {
        self.actor.send(request).await.into_precept_result()
    }
}
