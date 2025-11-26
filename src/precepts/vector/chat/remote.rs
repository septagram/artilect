use reqwest::RequestBuilder;

use crate::{
    precept::MessageRemoteStrategy,
    precepts::vector::chat::dto::{
        FetchThreadRequest, FetchUserThreadsRequest, SendMessageRequest,
    },
};

impl MessageRemoteStrategy for FetchUserThreadsRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.get(format!("{base_url}/chats"))
    }
}

impl MessageRemoteStrategy for FetchThreadRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.get(format!("{base_url}/chat/{}", self.thread_id))
    }
}

impl MessageRemoteStrategy for SendMessageRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.post(format!("{base_url}/chat")).json(&self)
    }
}
