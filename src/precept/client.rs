use std::sync::Arc;

use anyhow::Context;
use cfg_block::cfg_block;
use derive_more::From;
use serde::de::DeserializeOwned;

use super::{
    Error, Identity, IntoPreceptSpecificResultTyped, Message, SignedMessage, UnauthorizedError,
};
use crate::orchestra::PlexusClientBase;
#[cfg(feature = "client-http2")]
use crate::orchestra;

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

            pub fn to_client(&self, client_base: &PlexusClientBase) -> Result<ClientLocal<T>, crate::orchestra::Error> {
                Ok(ClientLocal::<T> {
                    addr: self.addr.clone(),
                    client_id: client_base.local_identity.ok_or(crate::orchestra::Error::NoLocalIdentity)?,
                })
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

        impl <T: actix::Actor> Eq for ClientLocal<T> {}

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
        use crate::orchestra::PlexusClientBaseRemote;
        pub use super::secret_provider::SecretProvider;

        #[derive(Clone, PartialEq)]
        pub struct AddrRemote {
            prefixed_url: Arc<url::Url>,
        }

        impl AddrRemote {
            pub fn new(prefixed_url: Arc<url::Url>) -> Self {
                Self { prefixed_url }
            }

            pub fn to_client(&self, client_base: &PlexusClientBase) -> Result<ClientRemote, orchestra::Error> {
                let Some(PlexusClientBaseRemote {
                    secret_provider,
                    reqwest_client,
                }) = client_base.remote
                else {
                    return crate::orchestra::Error::NoSecretProvider.into();
                };
                Ok(ClientRemote {
                    client: reqwest_client,
                    prefixed_url: self.prefixed_url.clone(),
                    secret_provider,
                })
            }
        }

        #[derive(Clone)]
        pub struct ClientRemote {
            client: reqwest::Client,
            prefixed_url: Arc<url::Url>,
            secret_provider: Arc<SecretProvider>,
        }

        impl ClientRemote {
            pub async fn send<S>(&self, msg: S) -> super::Result<S::Response>
            where
                S: super::MessageRemoteStrategy,
                S::Response: DeserializeOwned,
            {
                self.secret_provider
                    .ensure_valid_token(&self.client)
                    .await?;
                msg.into_request(&self.client, &*self.prefixed_url)
                    .with_context(|| format!("Failed to build request for {}", std::any::type_name::<S>()))?
                    .send()
                    .await
                    .into_precept_result_t::<S::Response>()
                    .await
            }
        }

        impl Eq for ClientRemote {}

        impl PartialEq for ClientRemote {
            fn eq(&self, other: &Self) -> bool {
                self.secret_provider == other.secret_provider && self.prefixed_url == other.prefixed_url
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

            pub fn new_remote(prefixed_url: Arc<url::Url>) -> Self {
                Self::Remote(AddrRemote::new(prefixed_url))
            }

            pub fn to_client(&self, client_base: &PlexusClientBase) -> Result<Client<P>, crate::orchestra::Error> {
                Ok(match self {
                    Self::Local(addr) => Client::Local(addr.to_client(client_base)?),
                    Self::Remote(addr) => Client::Remote(addr.to_client(client_base)?),
                })
            }
        }

        #[derive(Clone, PartialEq, Eq)]
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
