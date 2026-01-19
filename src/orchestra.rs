use std::{any::TypeId, collections::HashMap, sync::Arc};

use anyhow::Context;
use anymap3::AnyMap;
use bon::bon;
use cfg_block::cfg_block;
use thiserror::Error;
use url::Url;

use crate::{
    precept::{self, Identity, Precept, PreceptConstructor, PreceptID},
    precepts,
};

cfg_block! {
    #[cfg(feature = "client-http2")] {
        pub struct BaseUrl {
            pub base: url::Url,
            pub refresh_token: url::Url,
            pub login: url::Url,
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

#[cfg(feature = "backend")]
type InitCallback = Box<dyn FnOnce(&mut Plexus) -> Result<(), Error>>;

#[derive(Default)]
struct PlexusBuilder {
    #[cfg(feature = "backend")]
    l_precepts: AnyMap,
    #[cfg(feature = "backend")]
    l_init_callbacks: Vec<InitCallback>,

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
pub enum Error {
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
    #[cfg(feature = "backend")]
    #[error("Missing local identity in PlexusClientBase")]
    NoLocalIdentity,
    #[cfg(feature = "client-http2")]
    #[error("Missing secret provider in PlexusClientBase")]
    NoSecretProvider,
    #[cfg(feature = "backend")]
    #[error("Precept already initialized: {0:?}")]
    PreceptAlreadyInitialized(PreceptID),
}

impl PlexusBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "client-http2")]
    pub fn base_url(&mut self, base_url: BaseUrl) -> Result<&mut Self, Error> {
        self.r_base_url = Some(base_url);
        Ok(self)
    }

    pub fn take_base(&mut self) -> Result<PlexusBase, Error> {
        Ok(PlexusBase {
            #[cfg(feature = "client-http2")]
            base_url: self.r_base_url.take().ok_or(Error::NoBaseUrl)?,
            #[cfg(feature = "server-http2")]
            router: self.s_router.take().ok_or(Error::NoRouter)?,
        })
    }

    #[cfg(feature = "backend")]
    pub fn local_precept<T: PreceptConstructor>(
        &mut self,
        config: T::Config,
    ) -> Result<&mut Self, Error> {
        use precept::client::AddrLocal;

        let (addr, setter) = AddrLocal::<T>::new();
        self.l_precepts.insert(addr);

        // Capture expose info for router setup
        #[cfg(feature = "server-http2")]
        let expose_at = match &self.s_exposed_routers {
            ExposedRouters::None => None,
            ExposedRouters::Single(precept_type) if *precept_type == TypeId::of::<T>() => Some(None),
            ExposedRouters::Multiple(exposed_routers) => exposed_routers
                .get(&TypeId::of::<T>())
                .map(|prefix| Some(prefix.clone())),
            ExposedRouters::Single(_) => None, // Different precept type exposed
        };

        self.l_init_callbacks.push(Box::new(move |plexus| {
            #[cfg(feature = "server-http2")]
            use precept::local::Routable;

            let precept_id = T::id(&config);

            // 1. Create precept (reborrow as shared for injector)
            let precept = {
                let base = PlexusClientBase::builder()
                    .local_identity(Some(Identity::Precept(precept_id)))
                    .build();
                let injector = Injector::new(&*plexus, base);
                T::new(&injector, config)?
            };

            // 2. Build router from precept BEFORE starting (some routes need precept ref)
            #[cfg(feature = "server-http2")]
            let router_from_precept = expose_at.as_ref().map(|_| T::build_router(&precept));

            // 3. Start precept (consumes it)
            let actix_addr = precept.start();

            // 4. Set address
            setter.set(actix_addr.clone()).map_err(|_| {
                Error::PreceptAlreadyInitialized(precept_id)
            })?;

            // 5. Router setup (if exposed)
            #[cfg(feature = "server-http2")]
            if let Some(prefix) = expose_at {
                let mut router = router_from_precept.unwrap_or_default();
                router = router.merge(T::build_router(&actix_addr));
                match prefix {
                    None => plexus.base.router = router,
                    Some(prefix) => plexus.base.router = plexus.base.router.nest(&prefix, router),
                }
            }

            Ok(())
        }));

        #[cfg(feature = "server-http2")]
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
                precepts.insert(last, prefix);
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
        T::from_local(self)
            .or_else(|| T::from_remote(self, name))
            .ok_or(Error::NoPrecept)
    }

    pub fn build(mut self) -> Result<Plexus, Error> {
        #[cfg(feature = "backend")]
        let callbacks = std::mem::take(&mut self.l_init_callbacks);
        let mut plexus: Plexus = self.try_into()?;

        // Run all init callbacks now that Plexus is constructed
        #[cfg(feature = "backend")]
        for callback in callbacks {
            callback(&mut plexus)?;
        }

        Ok(plexus)
    }
}

trait SetupAddr: Sized {
    fn from_local(builder: &mut PlexusBuilder) -> Option<Self> {
        None
    }

    fn from_remote(builder: &mut PlexusBuilder, name: &'static str) -> Option<Self> {
        None
    }
}

#[cfg(feature = "backend")]
impl<T: PreceptConstructor> SetupAddr for precept::client::AddrLocal<T> {
    fn from_local(builder: &mut PlexusBuilder) -> Option<Self> {
        builder.l_precepts.remove::<Self>()
    }
}

#[cfg(feature = "client-http2")]
impl SetupAddr for precept::client::AddrRemote {
    fn from_remote(builder: &mut PlexusBuilder, name: &'static str) -> Option<Self> {
        builder.r_precept_addrs.remove(name)
    }
}

#[cfg(all(feature = "backend", feature = "client-http2"))]
impl<T: PreceptConstructor> SetupAddr for precept::client::Addr<T> {
    fn from_local(builder: &mut PlexusBuilder) -> Option<Self> {
        builder.l_precepts.remove::<precept::client::AddrLocal<T>>().map(Into::into)
    }

    fn from_remote(builder: &mut PlexusBuilder, name: &'static str) -> Option<Self> {
        builder.r_precept_addrs.remove(name).map(Into::into)
    }
}

struct PlexusBase {
    #[cfg(feature = "client-http2")]
    pub base_url: BaseUrl,
    #[cfg(feature = "server-http2")]
    pub router: axum::Router,
}

#[derive(PartialEq, Eq)]
pub struct PlexusClientBase {
    #[cfg(feature = "backend")]
    pub local_identity: Option<Identity>,
    #[cfg(feature = "client-http2")]
    pub remote: Option<PlexusClientBaseRemote>,
}

#[cfg(feature = "client-http2")]
pub struct PlexusClientBaseRemote {
    pub secret_provider: Arc<precept::client::SecretProvider>,
    pub reqwest_client: reqwest::Client,
}

#[cfg(feature = "client-http2")]
impl Eq for PlexusClientBaseRemote {}

#[cfg(feature = "client-http2")]
impl PartialEq for PlexusClientBaseRemote {
    fn eq(&self, other: &Self) -> bool {
        self.secret_provider == other.secret_provider
    }
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
        let remote = secret_provider.map(|secret_provider| PlexusClientBaseRemote {
            secret_provider: secret_provider.clone(),
            reqwest_client: reqwest::Client::builder()
                .cookie_provider(secret_provider.clone())
                .build()
                .unwrap(),
        });
        Self {
            #[cfg(feature = "backend")]
            local_identity,
            #[cfg(feature = "client-http2")]
            remote,
        }
    }
}

struct Plexus {
    base: PlexusBase,
    #[cfg(feature = "auth")]
    auth: precepts::auth::Addr,
    #[cfg(feature = "chat")]
    chat: precepts::chat::Addr,
    #[cfg(feature = "telegram")]
    telegram: precepts::telegram::Addr,
}

impl TryFrom<PlexusBuilder> for Plexus {
    type Error = Error;
    fn try_from(mut builder: PlexusBuilder) -> Result<Self, Error> {
        Ok(Self {
            #[cfg(feature = "auth")]
            auth: builder.take("auth")?,
            #[cfg(feature = "chat")]
            chat: builder.take("chat")?,
            #[cfg(feature = "telegram")]
            telegram: builder.take("telegram")?,
            base: builder.take_base()?,
        })
    }
}

impl Plexus {
    pub fn to_injector(&self, base: PlexusClientBase) -> Injector<'_> {
        Injector::new(self, base)
    }
}

pub struct Injector<'a> {
    base: PlexusClientBase,
    plexus: &'a Plexus,
}

impl<'a> Injector<'a> {
    fn new(plexus: &'a Plexus, base: PlexusClientBase) -> Self {
        Self { base, plexus }
    }

    #[cfg(feature = "auth")]
    pub fn auth(&self) -> Result<precepts::auth::Client, Error> {
        self.plexus.auth.to_client(&self.base)
    }

    #[cfg(feature = "chat")]
    pub fn chat(&self) -> Result<precepts::chat::Client, Error> {
        self.plexus.chat.to_client(&self.base)
    }

    #[cfg(feature = "telegram")]
    pub fn telegram(&self) -> Result<precepts::telegram::Client, Error> {
        self.plexus.telegram.to_client(&self.base)
    }
}

cfg_block! {
    #[cfg(feature = "frontend")] {
        use dioxus::prelude::*;

        // TODO: Implement alternative using per-precept hooks returning precept::client::Client(|Local|Remote)
        // pub fn use_plexus_client(plexus_client: &PlexusClient) {
        //     let mut plexus_client_signal = use_context_provider(|| Signal::new(plexus_client));
        //     use_effect(use_reactive!(|plexus_client| {
        //         let mut write = plexus_client_signal.write();
        //         *write = plexus_client;
        //     }));
        // }
    }
}
