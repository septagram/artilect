use std::sync::Arc;
use crate::precept;
use crate::precept::Identity;
use crate::precepts::vector::chat::dto::{FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest, FetchUserThreadsResponse, SendMessageRequest, SendMessageResponse};

pub struct GlobalChatClient {
    base_url: Arc<str>,
}

impl GlobalChatClient {
    pub fn to_client(&self, _client_id: Identity, token: Option<Arc<str>>) -> ChatClient {
        ChatClient { token, base_url: self.base_url.clone() }
    }
}
#[derive(Clone)]
pub struct ChatClient {
    token: Option<Arc<str>>,
    base_url: Arc<str>,
}

impl super::ChatClientTrait for ChatClient {
    async fn fetch_user_threads(self: &Self, request: FetchUserThreadsRequest) -> precept::Result<FetchUserThreadsResponse> {
        todo!()
    }
    
    async fn fetch_thread_messages(self: &Self, request: FetchThreadRequest) -> precept::Result<FetchThreadResponse> {
        todo!()
    }
    
    async fn chat(self: &Self, request: SendMessageRequest) -> precept::Result<SendMessageResponse> {
        todo!()
    }
}
