use std::sync::Arc;

use actix::Addr;
use tokio::sync::SetOnce;
use uuid::Uuid;

use crate::{
    precept,
    precept::{ActixResult, Identity, PreceptID, SignedMessage},
    precepts::vector::chat::{
        Precept as ChatPrecept,
        dto::{
            FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest,
            FetchUserThreadsResponse, SendMessageRequest, SendMessageResponse,
        },
    },
};

pub struct GlobalChatClient {
    pub addr: Arc<SetOnce<Addr<ChatPrecept>>>,
}

impl GlobalChatClient {
    pub fn to_client(&self, client_id: Identity, _token: Option<Arc<str>>) -> ChatClient {
        // ChatClient::new(client_id, self.addr.clone())
        ChatClient {
            client_id,
            addr: self.addr.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ChatClient {
    client_id: Identity,
    addr: Arc<SetOnce<Addr<ChatPrecept>>>,
}

impl ChatClient {
    pub fn with_user_id(&self, user_id: Uuid) -> Self {
        if !matches!(&self.client_id.precept_id, Some(precept_id) if *precept_id == PreceptID::Chat)
        {
            panic!("with_user_id can only be used by the same precept");
        }
        Self {
            client_id: Identity {
                user_id,
                precept_id: self.client_id.precept_id.clone(),
            },
            addr: self.addr.clone(),
        }
    }
}

impl super::ChatClientTrait for ChatClient {
    async fn fetch_user_threads(
        self: &Self,
        request: FetchUserThreadsRequest,
    ) -> precept::Result<FetchUserThreadsResponse> {
        self.addr
            .wait()
            .await
            .send(SignedMessage {
                from: self.client_id.clone(),
                data: request,
            })
            .await
            .map_actix_error()
    }

    async fn fetch_thread_messages(
        self: &Self,
        request: FetchThreadRequest,
    ) -> precept::Result<FetchThreadResponse> {
        self.addr
            .wait()
            .await
            .send(SignedMessage {
                from: self.client_id.clone(),
                data: request,
            })
            .await
            .map_actix_error()
    }

    async fn chat(
        self: &Self,
        request: SendMessageRequest,
    ) -> precept::Result<SendMessageResponse> {
        self.addr
            .wait()
            .await
            .send(SignedMessage {
                from: self.client_id.clone(),
                data: request,
            })
            .await
            .map_actix_error()
    }
}
