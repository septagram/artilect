use reqwest::RequestBuilder;
use url::{ParseError, Url};
use crate::{
    precept::MessageRemoteStrategy,
    precepts::chat::dto::{
        FetchThreadRequest, FetchUserThreadsRequest, SendMessageRequest,
    },
};

impl MessageRemoteStrategy for FetchUserThreadsRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.get(base_url.join("chats")?))
    }
}

impl MessageRemoteStrategy for FetchThreadRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.get(base_url.join(&*format!("chat/{}", self.thread_id))?))
    }
}

impl MessageRemoteStrategy for SendMessageRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.post(base_url.join("chat")?).json(&self))
    }
}
