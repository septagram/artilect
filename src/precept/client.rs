use std::sync::Arc;

use cfg_block::cfg_block;
use serde::de::DeserializeOwned;

use super::{Error, Identity, SignedMessage, UnauthorizedError};

cfg_block! {
    #[cfg(feature = "backend")] {
        use tokio::sync::SetOnce;

        #[derive(Clone)]
        pub struct AddrLocal<T: actix::Actor> {
            pub addr: Arc<SetOnce<actix::Addr<T>>>,
        }

        impl<T: actix::Actor> AddrLocal<T> {
            pub fn new() -> (Self, Arc<SetOnce<actix::Addr<T>>>) {
                let addr = Arc::new(SetOnce::new());
                (Self { addr: addr.clone() }, addr)
            }

            pub fn to_client(&self, client_id: Option<Identity>, _token: Option<Arc<str>>) -> ClientLocal<T> {
                ClientLocal::<T> {
                    addr: self.addr.clone(),
                    client_id: client_id.expect("Client ID must be set for local precepts"),
                }
            }
        }

        impl<T: actix::Actor> PartialEq for AddrLocal<T> {
            fn eq(&self, other: &Self) -> bool {
                self.addr == other.addr
            }
        }

        #[derive(Clone)]
        pub struct ClientLocal<T: actix::Actor> {
            addr: Arc<SetOnce<actix::Addr<T>>>,
            client_id: Identity,
        }

        impl<T> ClientLocal<T>
        where
            T: super::Precept,
        {
            pub async fn send<S>(&self, msg: S) -> super::Result<S::Response>
            where
                S: super::MessageLocalStrategy<T>,
                T: actix::Handler<SignedMessage<S>>,
                SignedMessage<S>: actix::Message<Result = super::Result<S::Response>>,
                // S: Send + Sync + 'static,
            {
                self.addr
                    .wait()
                    .await
                    .send(SignedMessage {
                        from: self.client_id.clone(),
                        data: msg,
                    })
                    .await
                    .map_actix_error()
            }
        }
        
        impl<T: actix::Actor> PartialEq for ClientLocal<T> {
            fn eq(&self, other: &Self) -> bool {
                self.addr == other.addr && self.client_id == other.client_id
            }
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
    }

    #[cfg(feature = "client-http2")] {
        use super::HttpErrorBodyBadRequest;

        #[derive(Clone, PartialEq)]
        pub struct AddrRemote {
            base_url: Arc<str>,
        }

        impl AddrRemote {
            pub fn new(base_url: Arc<str>) -> Self {
                Self { base_url }
            }

            pub fn to_client(&self, _client_id: Option<Identity>, token: Option<Arc<str>>) -> ClientRemote {
                ClientRemote { token, base_url: self.base_url.clone() }
            }
        }

        #[derive(Clone, PartialEq)]
        pub struct ClientRemote {
            token: Option<Arc<str>>,
            base_url: Arc<str>,
        }

        impl ClientRemote {
            pub async fn send<S>(&self, msg: S) -> super::Result<S::Response>
            where
                S: super::MessageRemoteStrategy,
                S::Response: DeserializeOwned,
            {
                let mut request = msg.into_request(self.base_url.as_ref());
                if let Some(token) = &self.token {
                    request = request.header("Authorization", format!("Bearer {}", token));
                }
                match request.send().await {
                    Ok(response) => {
                        let status = response.status();
                        if status.is_success() {
                            response.json::<S::Response>().await.map_err(|_| Error::InvalidResponse)
                        } else {
                            Err(match status.as_u16() {
                                400 => match response.json::<HttpErrorBodyBadRequest>().await {
                                    Ok(body) => Error::BadRequest(body.error),
                                    Err(_) => Error::InvalidResponse,
                                },
                                401 => match response.json::<UnauthorizedError>().await {
                                    Ok(body) => Error::Unauthorized(body),
                                    Err(_) => Error::InvalidResponse,
                                },
                                403 => Error::Forbidden,
                                404 => Error::NotFound,
                                500 => Error::Internal(anyhow::anyhow!("Internal error")),
                                501 => Error::NotImplemented,
                                503 => Error::ServiceUnavailable,
                                _ => Error::InvalidResponse
                            })
                        }
                    },
                    Err(error) => {
                        println!("{:?}", error);
                        Err(Error::ServiceUnavailable)
                        // if error.is_connect() {
                        //     Err(Error::ServiceUnavailable)
                        // } else {
                        //     Err(Error::InvalidResponse)
                        // }
                    },
                }
            }
        }
    }

    #[cfg(all(feature = "backend", feature = "client-http2"))] {
        #[derive(Clone, PartialEq)]
        pub enum Addr<P: actix::Actor> {
            Local(AddrLocal<P>),
            Remote(AddrRemote),
        }

        impl<P: actix::Actor> Addr<P> {
            pub fn new_local() -> (AddrLocal<P>, Arc<SetOnce<actix::Addr<P>>>) {
                AddrLocal::<P>::new()
            }

            pub fn new_remote(base_url: Arc<str>) -> AddrRemote {
                AddrRemote::new(base_url)
            }

            pub fn to_client(&self, client_id: Option<Identity>, token: Option<Arc<str>>) -> Client<P> {
                match self {
                    Self::Local(addr) => Client::Local(addr.to_client(client_id, token)),
                    Self::Remote(addr) => Client::Remote(addr.to_client(client_id, token)),
                }
            }
        }

        #[derive(Clone, PartialEq)]
        pub enum Client<P: actix::Actor> {
            Local(ClientLocal<P>),
            Remote(ClientRemote),
        }

        impl <P: super::Precept> Client<P> {
            pub async fn send<S>(&self, msg: S) -> super::Result<S::Response>
            where
                P: actix::Handler<SignedMessage<S>>,
                S: super::MessageLocalStrategy<P> + super::MessageRemoteStrategy,
                S::Response: DeserializeOwned,
                SignedMessage<S>: actix::Message<Result = super::Result<S::Response>>,
            {
                match self {
                    Self::Local(client) => client.send(msg).await,
                    Self::Remote(client) => client.send(msg).await,
                }
            }
        }
    }
}
