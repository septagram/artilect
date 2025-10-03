use reqwest::{Client, RequestBuilder};

use crate::{
    precept::MessageRemoteStrategy,
    precepts::vector::chat::dto::{
        FetchThreadRequest, FetchUserThreadsRequest, SendMessageRequest,
    },
};

impl MessageRemoteStrategy for FetchUserThreadsRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().get(format!("{base_url}/chats"))
    }
}

impl MessageRemoteStrategy for FetchThreadRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().get(format!("{base_url}/chat/{}", self.thread_id))
    }
}

impl MessageRemoteStrategy for SendMessageRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().post(format!("{base_url}/chat")).json(&self)
    }
}
