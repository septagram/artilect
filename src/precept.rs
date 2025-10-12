use serde::Deserialize;
use uuid::Uuid;
pub mod client;
#[cfg(feature = "backend")]
mod local;

#[cfg(feature = "backend")]
pub use local::*;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    #[error("Unauthorized")]
    Unauthorized,
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

pub struct SignedMessage<T> {
    pub from: Identity,
    pub data: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    User(Uuid),
    Service {
        id: PreceptID,
        on_behalf_of: Option<Uuid>,
    },
}

impl Identity {
    pub fn to_user_id(&self, allow_on_behalf: bool) -> Option<Uuid> {
        match self {
            Self::User(id) => Some(*id),
            Self::Service { on_behalf_of, .. } => match allow_on_behalf {
                true => *on_behalf_of,
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
struct HttpErrorBody {
    error: Box<str>,
}

#[cfg(feature = "server-http2")]
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Error::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, Some(msg)),
            Error::Unauthorized => (axum::http::StatusCode::UNAUTHORIZED, None),
            Error::Forbidden => (axum::http::StatusCode::FORBIDDEN, None),
            Error::NotFound => (axum::http::StatusCode::NOT_FOUND, None),
            Error::Internal(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, None),
            Error::InvalidResponse => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, None),
            Error::NotImplemented => (axum::http::StatusCode::NOT_IMPLEMENTED, None),
            Error::ServiceUnavailable => (axum::http::StatusCode::SERVICE_UNAVAILABLE, None),
        };

        match message {
            Some(error) => (status, axum::Json(HttpErrorBody { error })).into_response(),
            None => status.into_response(),
        }
    }
}
