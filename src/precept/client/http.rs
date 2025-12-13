use std::sync::Arc;

use anyhow::Context;
use cookie_store::CookieExpiration;
use http::HeaderValue;
use itertools::Itertools;
use keyring::Entry;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, Serializer};
use tokio::sync;
use url::Url;

use crate::{
    auth::middleware::make_access_token,
    precept::{Identity, PreceptID},
    util::report_err::*,
};
// @note: All of the below should've been like one line of code. Seriously. It's 2025.

pub const KEYRING_SERVICE_NAME: &str = "artilect";

#[derive(Debug, Default, Serialize, Deserialize)]
struct CurrentCookies {
    cookies: cookie_store::CookieStore,
    #[serde(serialize_with = "serialize_expiration")]
    access_token_exp: Option<CookieExpiration>,
}

pub enum SecretProvider {
    #[cfg(feature = "backend")]
    PreceptCookie { cookie_header: HeaderValue },
    #[cfg(feature = "client")]
    UserCookiesNative {
        artilect_base_url: Url,
        current_cookies: Mutex<CurrentCookies>,
        warnings_tx: sync::mpsc::Sender<anyhow::Error>,
    },
}

impl PartialEq for SecretProvider {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for SecretProvider {}

impl reqwest::cookie::CookieStore for SecretProvider {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        match self {
            #[cfg(feature = "backend")]
            Self::PreceptCookie { .. } => {} // Cookies cannot be modified for precepts.
            #[cfg(feature = "client")]
            Self::UserCookiesNative {
                artilect_base_url,
                current_cookies,
                warnings_tx,
            } => {
                let parse_cookie_err = || format!("Invalid cookie header for {url}");
                let add_cookie_err = || format!("Failed to add cookie for {url}");
                let save_cookie_err = || format!("Failed to save cookies for {url}");
                let mut lock = current_cookies.lock();
                let CurrentCookies {
                    ref mut cookies,
                    ref mut access_token_exp,
                } = *lock;
                let mut has_changes = false;
                for cookie_header in cookie_headers {
                    let cookie: Result<cookie_store::Cookie, anyhow::Error> = try {
                        let cookie_str = cookie_header //
                            .to_str()
                            .with_context(parse_cookie_err)?;
                        cookie_store::Cookie::parse(cookie_str, url)
                            .with_context(parse_cookie_err)?
                            .into_owned()
                    };
                    if let Some(cookie) = cookie.ok_or_report(warnings_tx) {
                        if cookie.name() == "at" {
                            *access_token_exp =
                                (!cookie.is_expired()).then(|| cookie.expires.clone())
                        }
                        cookies
                            .insert(cookie.clone(), url)
                            .map(|_| ())
                            .with_context(add_cookie_err)
                            .unwrap_or_report(warnings_tx);
                        has_changes = true;
                    };
                }
                if has_changes {
                    let saved: Result<(), anyhow::Error> = try {
                        let serialized_cookies =
                            serde_json::to_vec(&cookies).with_context(save_cookie_err)?;
                        Entry::new(KEYRING_SERVICE_NAME, artilect_base_url.as_str())
                            .and_then(|entry| entry.set_secret(&serialized_cookies))
                            .with_context(save_cookie_err)?
                    };
                }
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        match self {
            #[cfg(feature = "backend")]
            Self::PreceptCookie { cookie_header } => Some(cookie_header.clone()),
            #[cfg(feature = "client")]
            Self::UserCookiesNative {
                artilect_base_url: _,
                current_cookies,
                warnings_tx,
            } => {
                let all_cookies = current_cookies
                    .lock()
                    .cookies
                    .get_request_values(url)
                    .map(|(name, value)| cookie::Cookie::new(name, value).to_string())
                    .join("; ");
                match all_cookies.is_empty() {
                    true => None,
                    false => HeaderValue::try_from(all_cookies)
                        .with_context(|| format!("Failed to construct cookie header for {url}"))
                        .ok_or_report(warnings_tx)
                        .map(|mut header_value| {
                            header_value.set_sensitive(true);
                            header_value
                        }),
                }
            }
        }
    }
}

impl SecretProvider {
    #[cfg(feature = "backend")]
    fn new_precept(id: PreceptID) -> Result<Self, anyhow::Error> {
        let token = make_access_token(Identity::Precept {
            id,
            on_behalf_of: None,
        })
        .context("Failed to generate precept access token")?;
        let cookie_header_str = cookie::Cookie::new("at", &*token.token).to_string();
        let mut cookie_header = HeaderValue::try_from(cookie_header_str)
            .context("Failed to construct precept access token cookie header")?;
        cookie_header.set_sensitive(true);
        Ok(Self::PreceptCookie { cookie_header })
    }
    #[cfg(feature = "client")]
    fn new_user(
        artilect_base_url: Arc<str>,
    ) -> Result<(Self, sync::mpsc::Receiver<anyhow::Error>), anyhow::Error> {
        let (warnings_tx, warnings_rx) = sync::mpsc::channel(32);
        let current_cookies = get_current_cookies(&*artilect_base_url)
            .ok_or_report(&warnings_tx)
            .unwrap_or_default();
        Ok((
            Self::UserCookiesNative {
                artilect_base_url: Url::parse(&*artilect_base_url)
                    .with_context(|| format!("Invalid URL: {}", artilect_base_url))?,
                current_cookies: Mutex::new(current_cookies),
                warnings_tx,
            },
            warnings_rx,
        ))
    }
}

fn get_current_cookies(artilect_base_url: &str) -> anyhow::Result<CurrentCookies> {
    match Entry::new(KEYRING_SERVICE_NAME, &*artilect_base_url).and_then(|entry| entry.get_secret())
    {
        Ok(secret) => Ok(serde_json::from_slice::<CurrentCookies>(&*secret)?),
        Err(keyring::Error::NoEntry) => Ok(CurrentCookies::default()),
        Err(err) => Err(anyhow::anyhow!(err)),
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    cookie_store: Arc<SecretProvider>,
}

impl HttpClient {
    pub fn new(cookie_store: SecretProvider) -> Self {
        let cookie_store = Arc::new(cookie_store);
        let client = reqwest::Client::builder()
            .cookie_provider(cookie_store.clone())
            .build()
            .unwrap();
        Self {
            client,
            cookie_store,
        }
    }

    pub async fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

impl PartialEq for HttpClient {
    fn eq(&self, other: &Self) -> bool {
        self.cookie_store == other.cookie_store
    }
}

fn serialize_expiration<S>(exp: &Option<CookieExpiration>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match exp {
        Some(CookieExpiration::AtUtc(dt)) => serializer.serialize_some(dt),
        Some(CookieExpiration::SessionEnd) | None => serializer.serialize_none(),
    }
}
