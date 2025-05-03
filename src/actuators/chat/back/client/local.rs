use std::sync::Arc;

use actix::Addr;

use crate::{
    actuators::chat::{
        back::actor::ChatService,
        dto::{
            FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse,
            SendMessageRequest, SendMessageResponse,
        }
    },
    service,
    service::ActixResult,
};

#[derive(Clone)]
pub struct ChatClient {
    actor: Arc<Addr<ChatService>>,
}

impl ChatClient {
    pub fn new(addr: Arc<Addr<ChatService>>) -> Self {
        Self { actor: addr }
    }
}

impl super::ChatClientTrait for ChatClient {
    async fn fetch_user_threads(self: &Self, request: FetchUserThreadsRequest) -> service::Result<FetchUserThreadsResponse> {
        self.actor.send(request).await.into_service_result()
    }

    async fn fetch_thread_messages(self: &Self, request: FetchThreadRequest) -> service::Result<FetchThreadResponse> {
        self.actor.send(request).await.into_service_result()
    }

    async fn chat(self: &Self, request: SendMessageRequest) -> service::Result<SendMessageResponse> {
        self.actor.send(request).await.into_service_result()
    }
}
