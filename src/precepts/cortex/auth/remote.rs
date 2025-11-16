use reqwest::{Client, RequestBuilder};
use crate::precept::MessageRemoteStrategy;
use super::dto::*;

impl MessageRemoteStrategy for ConfirmLoginRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().put(format!("{base_url}/svc/attempt")).json(&self)
    }
}

impl MessageRemoteStrategy for InvalidateLoginRequest {
    fn into_request(self, base_url: &str) -> RequestBuilder {
        Client::new().delete(format!("{base_url}/svc/attempt"))
    }
}
