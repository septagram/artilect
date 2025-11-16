use std::sync::Arc;
use serde::Deserialize;
use uuid::Uuid;
pub mod client;
#[cfg(feature = "backend")]
pub mod local;

#[cfg(feature = "backend")]
pub use local::*;
use serde::{Serialize, de::DeserializeOwned};
use crate::auth::User;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreceptID {
    #[cfg(any(feature = "auth-in", feature = "auth-out"))]
    Auth,
    #[cfg(any(feature = "chat-in", feature = "chat-out"))]
    Chat,
    #[cfg(any(feature = "telegram-in", feature = "telegram-out"))]
    Telegram,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bad Request: {0}")]
    BadRequest(Box<str>),
    #[error("Unauthorized: {0}")]
    Unauthorized(#[from] UnauthorizedError),
    #[error("Forbidden")]
    Forbidden,
    #[error("Not Found")]
    NotFound,
    #[error("Precept Unavailable")]
    ServiceUnavailable,
    #[error("Invalid Response")]
    InvalidResponse,
    #[error("Not Implemented")]
    NotImplemented,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "server-http2", derive(Serialize))]
#[cfg_attr(feature = "client-http2", derive(Deserialize))]
pub enum UnauthorizedError {
    #[error("Missing authentication")]
    Missing,
    #[error("Access token has expired")]
    ExpiredToken,
    #[error("Invalid access token")]
    InvalidToken,
    #[error("Invalid session")]
    InvalidSession,
}

pub struct SignedMessage<T> {
    pub from: Identity,
    pub data: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UserIdentity {
    user_id: Uuid,
    // account_id: Uuid,
    // is_operator: bool,
    // or role: Role, // derives Copy
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Identity {
    User (UserIdentity),
    Precept {
        id: PreceptID,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_behalf_of: Option<UserIdentity>,
    },
}

impl Identity {
    pub fn to_precept_id(&self) -> Option<PreceptID> {
        match self {
            Self::Precept { id, .. } => Some(*id),
            _ => None,
        }
    }

    pub fn to_user_id(&self, allow_on_behalf: bool) -> Option<Uuid> {
        match self {
            Self::User(user_identity) => Some(user_identity.user_id),
            Self::Precept { on_behalf_of, .. } => match allow_on_behalf {
                true => on_behalf_of.as_ref().map(|user_identity| user_identity.user_id),
                false => None,
            },
        }
    }
}

pub trait Message: Send + Sync + 'static {
    type Response: Send + Sync + 'static;
}

#[cfg(feature = "backend")]
pub trait MessageLocalStrategy<P: Precept>: Message {
    #[cfg(feature = "server-http2")]
    fn route(router: axum::Router<actix::Addr<P>>) -> axum::Router<actix::Addr<P>>;
    fn handle(
        resources: &P::Resources,
        state: &P::State,
        from: Identity,
        message: Self,
    ) -> impl Future<Output = Result<Self::Response>>;
}

#[cfg(feature = "client-http2")]
pub trait MessageRemoteStrategy: Message {
    fn into_request(self, base_url: &str) -> reqwest::RequestBuilder;
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait CoercibleResult<T> {
    fn into_precept_result(self: Self) -> Result<T>;
}

impl<T, E> CoercibleResult<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_precept_result(self: Self) -> Result<T> {
        self.map_err(|e| anyhow::Error::from(e).into())
    }
}

#[cfg(any(feature = "server-http2", feature = "client-http2"))]
#[cfg_attr(feature = "server-http2", derive(Serialize))]
#[cfg_attr(feature = "client-http2", derive(Deserialize))]
struct HttpErrorBodyBadRequest {
    error: Box<str>,
}

#[cfg(feature = "server-http2")]
enum HttpErrorDetail {
    Unauthorized(UnauthorizedError),
    BadRequest(Box<str>),
}

#[cfg(feature = "server-http2")]
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_details) = match self {
            Error::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, Some(HttpErrorDetail::BadRequest(msg))),
            Error::Unauthorized(detail) => (axum::http::StatusCode::UNAUTHORIZED, Some(HttpErrorDetail::Unauthorized(detail))),
            Error::Forbidden => (axum::http::StatusCode::FORBIDDEN, None),
            Error::NotFound => (axum::http::StatusCode::NOT_FOUND, None),
            Error::Internal(err) => {
                tracing::error!("{:?}", err);
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, None)
            },
            Error::InvalidResponse => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, None),
            Error::NotImplemented => (axum::http::StatusCode::NOT_IMPLEMENTED, None),
            Error::ServiceUnavailable => (axum::http::StatusCode::SERVICE_UNAVAILABLE, None),
        };

        use HttpErrorDetail as D;
        match error_details {
            Some(details) => match details {
                D::BadRequest(error) => (status, axum::Json(HttpErrorBodyBadRequest { error })).into_response(),
                D::Unauthorized(error) => (status, axum::Json(error)).into_response(), // @todo: improve
            },
            None => status.into_response(),
        }
    }
}
