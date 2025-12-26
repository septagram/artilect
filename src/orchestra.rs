use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use anyhow::Context;
use anymap3::AnyMap;
use bon::bon;
use cfg_block::cfg_block;
use itertools::Itertools;
use thiserror::Error;
use tokio::sync::{SetOnce, mpsc};
use url::Url;

use crate::{
    precept::{self, Identity, Precept, PreceptConstructor},
    precepts,
};

cfg_block! {
    #[cfg(feature = "client-http2")] {
        pub struct BaseUrl {
            base: url::Url,
            refresh_token: url::Url,
            login: url::Url,
        }

        impl BaseUrl {
            fn new(base_url: url::Url, auth_base_url: Option<url::Url>) -> Result<Self, anyhow::Error> {
                let context = |step: &str| {
                    let base_url = base_url.as_str();
                    move || format!("Failed to generate {} URL for {}", step, base_url)
                };
                let auth_base_url = auth_base_url
                    .ok_or_else(|| base_url.join("auth/"))
                    .with_context(context("auth base"))?;
                let refresh_token_url = auth_base_url
                    .join("refresh")
                    .with_context(context("refresh token"))?;
                let login_url = auth_base_url.join("login").with_context(context("login"))?;

                Ok(Self {
                    base: base_url,
                    refresh_token: refresh_token_url,
                    login: login_url,
                })
            }
        }
    }
}

// trait OrchestraExt {
//     type Builder: OrchestraBuilderExt;
//     fn build(self) -> Self::Builder;
//     type AddressBook: Clone + Send + Sync + PartialEq + Eq + 'static;
//     fn to_address_book(&self, identity: Option<Identity>) -> Self::AddressBook;
// }

#[cfg(feature = "server-http2")]
enum ExposedRouters {
    None,
    Single(TypeId),
    Multiple(HashMap<TypeId, Box<str>>),
}

#[cfg(feature = "server-http2")]
impl Default for ExposedRouters {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
struct OrchestraBuilder {
    #[cfg(feature = "backend")]
    l_precepts: AnyMap,

    #[cfg(feature = "server-http2")]
    s_last: Option<TypeId>,
    #[cfg(feature = "server-http2")]
    s_exposed_routers: ExposedRouters,
    #[cfg(feature = "server-http2")]
    s_router: Option<axum::Router>,

    #[cfg(feature = "client-http2")]
    r_base_url: Option<BaseUrl>,
    #[cfg(feature = "client-http2")]
    r_precept_addrs: HashMap<&'static str, precept::client::AddrRemote>,
}

#[derive(Error, Debug)]
enum Error {
    #[error("No base URL provided or base URL already taken")]
    NoBaseUrl,
    #[error("Missing precept")]
    NoPrecept,
    #[error("Router taken, change initiation order")]
    NoRouter,
    #[error("No precepts to expose")]
    NoPreceptsToExpose,
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl OrchestraBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "client-http2")]
    pub fn base_url(&mut self, base_url: BaseUrl) -> Result<&mut Self, Error> {
        self.r_base_url = Some(base_url);
        Ok(self)
    }

    pub fn take_base(&mut self) -> Result<PlexusBase, Error> {
        PlexusBase {
            #[cfg(feature = "client-http2")]
            base_url: self.r_base_url.take().ok_or(Error::NoBaseUrl)?,
            #[cfg(feature = "server-http2")]
            router: self.s_router.take().ok_or(Error::NoRouter)?,
        }
        .into()
    }

    #[cfg(feature = "backend")]
    pub fn local_precept<T: PreceptConstructor>(
        &mut self,
        config: T::Config,
    ) -> Result<&mut Self, Error> {
        pub use precept::client::AddrLocal;
        self.l_precepts.insert(config);
        // let (tx, rx) = mpsc::channel::<T::Message>(128);
        // self.local_precepts.insert(tx);
        // self.local_precepts.insert(rx);
        // ^ if removing Actix
        let (addr, set_addr) = AddrLocal::<T>::new();
        self.l_precepts.insert(addr);
        self.l_precepts.insert(set_addr);
        self.s_last = Some(TypeId::of::<T>());
        Ok(self)
    }

    #[cfg(feature = "server-http2")]
    pub fn expose_at(&mut self, prefix: Box<str>) -> Result<&mut Self, Error> {
        let last = self.s_last.ok_or(Error::NoPreceptsToExpose)?;
        match &mut self.s_exposed_routers {
            ExposedRouters::Multiple(precepts) => {
                precepts.insert(last, prefix);
            }
            _ => {
                let mut precepts = HashMap::new();
                precepts.insert(prefix, last);
                self.s_exposed_routers = ExposedRouters::Multiple(precepts);
            }
        };
        Ok(self)
    }

    #[cfg(feature = "server-http2")]
    pub fn expose_only(&mut self) -> Result<&mut Self, Error> {
        self.s_exposed_routers =
            ExposedRouters::Single(self.s_last.ok_or(Error::NoPreceptsToExpose)?);
        Ok(self)
    }

    #[cfg(feature = "client-http2")]
    pub fn remote_precept(
        &mut self,
        name: &'static str,
        prefix: Option<Url>,
    ) -> Result<&mut Self, Error> {
        let prefix = match prefix {
            Some(value) => value,
            None => self
                .r_base_url
                .as_ref()
                .ok_or(Error::NoBaseUrl)?
                .base
                .join(name)
                .context("Failed to make prefixed URL")?,
        };
        self.r_precept_addrs
            .insert(name, precept::client::AddrRemote::new(Arc::new(prefix)));
        Ok(self)
    }

    fn take<T: SetupAddr>(&mut self, name: &'static str) -> Result<T, Error> {
        T::from_local(self)?
            .ok_or_else(|| T::from_remote(self, name))
            .ok_or(Error::NoPrecept)
    }

    pub fn build(self) -> Result<Orchestra, Error> {
        self.into()
    }
}

trait SetupAddr {
    fn from_local(builder: &mut OrchestraBuilder) -> Result<Option<Self>, anyhow::Error> {
        Ok(None)
    }

    fn from_remote(builder: &mut OrchestraBuilder, name: &'static str) -> Option<Self> {
        None
    }
}

#[cfg(feature = "backend")]
impl<T: PreceptConstructor> SetupAddr for precept::client::AddrLocal<T> {
    fn from_local(builder: &mut OrchestraBuilder) -> Result<Option<Self>, anyhow::Error> {
        addr_from_local_precept(builder)
    }
}

#[cfg(feature = "client-http2")]
impl SetupAddr for precept::client::AddrRemote {
    fn from_remote(builder: &mut OrchestraBuilder, name: &'static str) -> Option<Self> {
        builder.r_precept_addrs.remove(name)
    }
}

#[cfg(all(feature = "backend", feature = "client-http2"))]
impl<T: PreceptConstructor> SetupAddr for precept::client::Addr<T> {
    fn from_local(builder: &mut OrchestraBuilder) -> Result<Option<Self>, anyhow::Error> {
        addr_from_local_precept(builder).into()
    }

    fn from_remote(builder: &mut OrchestraBuilder, name: &'static str) -> Option<Self> {
        builder.r_precept_addrs.remove(name).into()
    }
}

#[cfg(feature = "backend")]
fn addr_from_local_precept<T: PreceptConstructor>(
    builder: &mut OrchestraBuilder,
) -> Result<Option<T::Addr>, anyhow::Error> {
    use precept::client::AddrLocal;
    #[cfg(feature = "server-http2")]
    use precept::local::Routable;

    let Some(precept) = builder.l_precepts.remove::<T>() else {
        return Ok(None);
    };
    let expose_at = match builder.s_exposed_routers {
        ExposedRouters::None => None,
        ExposedRouters::Single(precept_type) if precept_type == TypeId::of::<T>() => Some(None),
        ExposedRouters::Multiple(mut exposed_routers) => exposed_routers
            .remove(&TypeId::of::<T>())
            .map(|prefix| Some(prefix)),
        _ => None,
    };
    #[cfg(feature = "server-http2")]
    let mut router = (&expose_at)
        .map(|_| T::build_router(&precept))
        .unwrap_or_default();
    let actix_addr = precept.start();
    #[cfg(feature = "server-http2")]
    if let Some(prefix) = expose_at {
        router = router.merge(T::build_router(&actix_addr));
        match prefix {
            None => *builder.s_router.as_mut().ok_or(Error::NoRouter) = router,
            Some(prefix) => {
                *builder
                    .s_router
                    .as_mut()
                    .ok_or(Error::NoRouter)
                    .nest(&*prefix, router);
            }
        }
    }
    let err = |s: &'static str| || anyhow::anyhow!(s);
    builder
        .l_precepts
        .remove::<Arc<SetOnce<actix::Addr<T>>>>()
        .ok_or_else(err("Failed to get local precept Actix address setter"))?
        .set(actix_addr)
        .ok_or_else(err("Failed to set local precept address"))?;
    let addr = builder
        .l_precepts
        .remove::<AddrLocal<T>>()
        .ok_or_else(err("Failed to get local precept address"))?;
    Some(addr.into())
}

struct PlexusBase {
    #[cfg(feature = "client-http2")]
    pub base_url: BaseUrl,
    #[cfg(feature = "server-http2")]
    pub router: axum::Router,
}

pub struct PlexusClientBase {
    #[cfg(feature = "backend")]
    pub local_identity: Option<Identity>,
    #[cfg(feature = "client-http2")]
    pub secret_provider: Option<Arc<precept::client::SecretProvider>>,
    #[cfg(feature = "client-http2")]
    pub reqwest_client: Option<reqwest::Client>,
}

#[bon]
impl PlexusClientBase {
    #[builder]
    pub fn new(
        #[cfg(feature = "backend")] local_identity: Option<Identity>,
        #[cfg(feature = "client-http2")] secret_provider: Option<
            Arc<precept::client::SecretProvider>,
        >,
    ) -> Self {
        #[cfg(feature = "client-http2")]
        let reqwest_client = secret_provider.as_ref().map(|secret_provider| {
            reqwest::Client::builder()
                .cookie_provider(secret_provider.clone())
                .build()
                .unwrap()
        });
        Self {
            #[cfg(feature = "backend")]
            local_identity,
            #[cfg(feature = "client-http2")]
            secret_provider,
            #[cfg(feature = "client-http2")]
            reqwest_client,
        }
    }
}

// trait OrchestraBuilderExt {
//     #[cfg(feature = "client-http2")]
//     fn base_url(&self, base_url: &str) -> Self;
// }
//
// Use anymap to store the builder state, the precepts themselves, the MSPC txs and rxs.
// Construct the address book first.
// Builder methods are separate for local and remote precepts, the local ones accept config, remote
// ones accept prefix URL.
// Only the final AddressBook avoids anymap, has Client fields directly.

// trait OrchestraBuilderSetPrecept: OrchestraBuilderExt {}
//
// #[derive(OrchestraExt)]
struct Orchestra {
    base: PlexusBase,
    #[cfg(feature = "auth")]
    auth: precepts::auth::Addr,
    #[cfg(feature = "chat")]
    chat: precepts::chat::Addr,
    #[cfg(feature = "telegram")]
    telegram: precepts::telegram::Addr,
}

impl TryFrom<OrchestraBuilder> for Orchestra {
    type Error = Error;
    fn try_from(mut builder: OrchestraBuilder) -> Result<Self, Error> {
        Self {
            #[cfg(feature = "auth")]
            auth: builder.take("auth")?,
            #[cfg(feature = "chat")]
            chat: builder.take("chat")?,
            #[cfg(feature = "telegram")]
            telegram: builder.take("telegram")?,
            base: builder.take_base()?,
        }
        .into()
    }
}

// orchestra_from_precepts! {
//     auth: auth,
//     chat: chat,
//     telegram: telegram,
//     // valid ignored comment
// }

cfg_block! {
    #[cfg(feature = "frontend")] {
        use dioxus::prelude::*;

        pub fn use_address_book(orchestra: Arc<Orchestra>, identity: Option<Identity>) {
            let mut address_book = use_context_provider(|| Signal::new(orchestra.to_address_book(identity.clone())));
            use_effect(use_reactive!(|orchestra, identity| {
                let mut write = address_book.write();
                *write = orchestra.to_address_book(identity);
            }));
        }
    }
}
