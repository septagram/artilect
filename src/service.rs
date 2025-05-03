use actix::MailboxError;
use serde::Deserialize;

#[cfg(feature = "backend")]
#[allow(unused_imports)]
use serde::Serialize;

pub enum ServiceType {
    #[cfg(any(feature = "auth-in", feature = "auth-out"))]
    Auth,
    #[cfg(any(feature = "chat-in", feature = "chat-out"))]
    ChatActuator,
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
    #[error("Service Unavailable")]
    ServiceUnavailable,
    #[error("Invalid Response")]
    InvalidResponse,
    #[error("Not Implemented")]
    NotImplemented,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait CoercibleResult<T> {
    fn into_service_result(self: Self) -> Result<T>;
}

impl<T, E> CoercibleResult<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_service_result(self: Self) -> Result<T> {
        self.map_err(|e| anyhow::Error::from(e).into())
    }
}

pub trait ActixResult<T> {
    fn into_service_result(self: Self) -> Result<T>;
}

impl<T> ActixResult<T> for std::result::Result<Result<T>, MailboxError> {
    fn into_service_result(self: Self) -> Result<T> {
        match self {
            Ok(service_response) => match service_response {
                Ok(response) => Ok(response),
                Err(error) => {
                    tracing::error!("Service error: {:?}", error);
                    Err(error)
                },
            },
            Err(error) => {
                tracing::error!("Mailbox error: {:?}", error);
                Err(Error::ServiceUnavailable)
            },
        }
    }
}

// fn map_service_response<T>(
//     actix_response: Result<service::Result<T>, MailboxError>,
// ) -> service::Result<Json<T>> {
//     match actix_response {
//         Ok(service_response) => match service_response {
//             Ok(response) => Ok(Json(response)),
//             Err(error) => {
//                 tracing::error!("Service error: {:?}", error);
//                 Err(error)
//             },
//         },
//         Err(err) => {
//             tracing::error!("Mailbox error: {:?}", err);
//             Err(service::Error::ServiceUnavailable)
//         },
//     }
// }

#[cfg(feature = "backend")]
impl From<actix::MailboxError> for Error {
    fn from(_: actix::MailboxError) -> Self {
        Error::ServiceUnavailable
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
            None => status.into_response()
        }
    }
}

// #[cfg(feature = "client-http2")]
// impl From<reqwest::Error> for Error {
//     fn from(error: reqwest::Error) -> Self {
//         if error.is_connect() {
//             Error::ServiceUnavailable
//         } else if let Some(status) = error.status() {
//             match status.as_u16() {
//                 // 400 => ServiceError::BadRequest(error.json::<serde_json::Value>().await.map_or_else(
//                 //     |_| error.to_string().into(),
//                 //     |json| json.get("error").and_then(|e| e.as_str()).unwrap_or_default().into()
//                 // )),
//                 400 => Error::BadRequest(Box::from("(parsing not implemented)")),
//                 401 => Error::Unauthorized,
//                 403 => Error::Forbidden,
//                 404 => Error::NotFound,
//                 500 => Error::Internal,
//                 501 => Error::NotImplemented,
//                 503 => Error::ServiceUnavailable,
//                 _ => Error::InvalidResponse
//             }
//         } else {
//             Error::InvalidResponse
//         }
//     }
// }
