use super::{Error, Message, PreceptID, SignedMessage};

pub trait Precept: actix::Actor<Context = actix::Context<Self>> {
    // const ID: PreceptID;
    // const ROUTE_PREFIX: &'static str;
    type Resources;
    // #[cfg(feature = "server-http2")]
    // fn build_router(addr: actix::Addr<Self>) -> axum::Router;
    // fn new(resources: Self::Resources) -> Self;
}

impl <M> actix::Message for SignedMessage<M>
where
    M: Message,
{
    type Result = super::Result<M::Response>;
}

pub trait ActixResult<T> {
    fn map_actix_error(self: Self) -> super::Result<T>;
}

impl<T> ActixResult<T> for Result<super::Result<T>, actix::MailboxError> {
    fn map_actix_error(self: Self) -> super::Result<T> {
        match self {
            Ok(precept_response) => match precept_response {
                Ok(response) => Ok(response),
                Err(error) => {
                    tracing::error!("Precept error: {:?}", error);
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

impl From<actix::MailboxError> for Error {
    fn from(_: actix::MailboxError) -> Self {
        Error::ServiceUnavailable
    }
}

#[cfg(feature = "server-http2")]
pub trait Routable {
    fn build_router(self) -> axum::Router;
}
