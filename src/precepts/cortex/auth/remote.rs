use reqwest::RequestBuilder;
use crate::precept::MessageRemoteStrategy;
use super::dto::*;

impl MessageRemoteStrategy for ConfirmLoginRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.post(format!("{base_url}/svc/attempt/confirm")).json(&self)
    }
}

impl MessageRemoteStrategy for InvalidateLoginRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.post(format!("{base_url}/svc/attempt/invalidate")).json(&self)
    }
}

impl MessageRemoteStrategy for BotLoginStartRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        let flow_id = self.flow_id.as_str();
        client.post(format!("{base_url}/login/bot/{flow_id}")).json(&self)
    }
}

impl MessageRemoteStrategy for LoginPollRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.post(format!("{base_url}/login/poll")).json(&self)
    }
}

impl MessageRemoteStrategy for ListAuthProvidersRequest {
    fn into_request(self, client: &reqwest::Client, base_url: &str) -> RequestBuilder {
        client.get(format!("{base_url}/providers"))
    }
}
