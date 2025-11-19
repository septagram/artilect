use reqwest::{Client, RequestBuilder};
use crate::precept::MessageRemoteStrategy;
use super::dto::*;

impl MessageRemoteStrategy for ConfirmLoginRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().post(format!("{base_url}/svc/attempt/confirm")).json(&self)
    }
}

impl MessageRemoteStrategy for InvalidateLoginRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().post(format!("{base_url}/svc/attempt/invalidate")).json(&self)
    }
}

impl MessageRemoteStrategy for TelegramLoginStartRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().post(format!("{base_url}/login/telegram")).json(&self)
    }
}

impl MessageRemoteStrategy for LoginPollRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().post(format!("{base_url}/login/poll")).json(&self)
    }
}
