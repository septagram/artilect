use reqwest::RequestBuilder;
use url::{ParseError, Url};
use crate::precept::MessageRemoteStrategy;
use super::dto::*;

impl MessageRemoteStrategy for ConfirmLoginRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.post(base_url.join("svc/attempt/confirm")?).json(&self))
    }
}

impl MessageRemoteStrategy for InvalidateLoginRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.post(base_url.join("svc/attempt/invalidate")?).json(&self))
    }
}

impl MessageRemoteStrategy for BotLoginStartRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        let flow_id = self.flow_id.as_str();
        Ok(client.post(base_url.join(&*format!("login/bot/{flow_id}"))?).json(&self))
    }
}

impl MessageRemoteStrategy for LoginPollRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.post(base_url.join("login/poll")?).json(&self))
    }
}

impl MessageRemoteStrategy for ListAuthProvidersRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &Url) -> Result<RequestBuilder, ParseError> {
        Ok(client.get(base_url.join("providers")?))
    }
}
