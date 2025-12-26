use std::sync::Arc;

use cfg_block::cfg_block;
use serde::de::DeserializeOwned;
use derive_more::From;
use crate::orchestra::{PlexusClientBase, PlexusClientConfig};
use super::{Error, Identity, IntoPreceptSpecificResultTyped, Message, SignedMessage, UnauthorizedError};

pub enum OnlyRemote {}

// trait AddrTrait {
//     fn send<S: Message>(&self, msg: S) -> impl Future<Output = super::Result<S::Response>>;
//     // that's actually a client, not an addr
// }

cfg_block! {
    #[cfg(feature = "backend")] {
        use tokio::sync::SetOnce;

        #[cfg(not(feature = "client-http2"))]
        type HttpClient = ();

        #[derive(Clone)]
        pub struct AddrLocal<T: actix::Actor> {
            pub addr: Arc<SetOnce<actix::Addr<T>>>,
        }

        impl<T: actix::Actor> AddrLocal<T> {
            pub fn new() -> (Self, Arc<SetOnce<actix::Addr<T>>>) {
                let addr = Arc::new(SetOnce::new());
                (Self { addr: addr.clone() }, addr)
            }

            pub fn to_client(&self, client_id: Option<Identity>, _client: Option<HttpClient>) -> ClientLocal<T> {
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
        mod http;
        pub use http::HttpClient;
        pub use http::SecretProvider;

        #[derive(Clone, PartialEq)]
        pub struct AddrRemote {
            prefixed_url: Arc<url::Url>,
        }

        impl AddrRemote {
            pub fn new(prefixed_url: Arc<url::Url>) -> Self {
                Self { prefixed_url }
            }

            pub fn to_client(&self, client_base: &PlexusClientBase) -> ClientRemote {
                ClientRemote {
                    client: HttpClient::new(self.prefixed_url, client_base)
                }
            }
        }

        #[derive(Clone, PartialEq)]
        pub struct ClientRemote {
            client: HttpClient,
        }

        impl ClientRemote {
            pub async fn send<S>(&self, msg: S) -> super::Result<S::Response>
            where
                S: super::MessageRemoteStrategy,
                S::Response: DeserializeOwned,
            {
                self.client.send(msg).await
                // let request = msg.into_request(self.client.client().await, self.base_url.as_ref());
                // request.send().await.into_precept_result_t::<S::Response>().await
            }
        }
    }

    #[cfg(all(feature = "backend", feature = "client-http2"))] {
        #[derive(Clone, PartialEq, From)]
        pub enum Addr<P: actix::Actor> {
            Local(AddrLocal<P>),
            Remote(AddrRemote),
        }

        impl<P: actix::Actor> Addr<P> {
            pub fn new_local() -> (Self, Arc<SetOnce<actix::Addr<P>>>) {
                let (addr, set_addr) = AddrLocal::<P>::new();
                (Self::Local(addr), set_addr)
            }

            pub fn new_remote(base_url: Arc<str>) -> Self {
                Self::Remote(AddrRemote::new(base_url))
            }

            pub fn to_client(&self, client_id: Option<Identity>, client: Option<HttpClient>) -> Client<P> {
                match self {
                    Self::Local(addr) => Client::Local(addr.to_client(client_id, client)),
                    Self::Remote(addr) => Client::Remote(addr.to_client(client_id, client)),
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
