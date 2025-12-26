use std::{mem, sync::Arc};

use anyhow::Context;
use cookie_store::{self, CookieExpiration};
use itertools::Itertools;
use jsonwebtoken::EncodingKey;
use keyring::Entry;
use parking_lot::Mutex;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use time::{Duration, OffsetDateTime, UtcDateTime, UtcOffset};
use tokio::sync::mpsc;
use url::Url;

#[cfg(feature = "backend")]
use crate::auth::middleware::JwtClaims;
use crate::{
    auth::dto::RefreshTokenApiResponse,
    precept,
    precept::{
        Identity, IntoPreceptResult, IntoPreceptSpecificResultTyped, PreceptID, UnauthorizedError,
    },
    util::report_err::*,
};
// @note: All of the below should've been like one line of code. Seriously. It's 2025.
//
// ...to be fair, there's plenty of custom logic here...

pub const KEYRING_SERVICE_NAME: &str = "artilect";

struct BaseUrl {
    base: Url,
    refresh_token: Url,
    login: Url,
}

impl BaseUrl {
    fn new(base_url: Url, auth_base_url: Option<Url>) -> Result<Self, anyhow::Error> {
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

struct HeaderExpPair {
    header: HeaderValue,
    exp: CookieExpiration,
}

enum ClientSecrets {
    #[cfg(feature = "backend")]
    PreceptCookie {
        precept_id: PreceptID,
        exp_duration: Duration,
        encoding_key: Arc<EncodingKey>,
        cookie_header: Option<HeaderExpPair>,
    },
    #[cfg(feature = "native")]
    UserCookiesNative {
        base_url: BaseUrl,
        cookies: cookie_store::CookieStore,
        access_token_exp: Option<CookieExpiration>,
        refresh_token_exp: Option<CookieExpiration>,
    },
}

#[derive(Serialize, Deserialize)]
#[cfg(feature = "native")]
struct KeyringSecretStorage {
    cookies: cookie_store::CookieStore,
    #[serde(serialize_with = "serialize_expiration")]
    access_token_exp: Option<CookieExpiration>,
    #[serde(serialize_with = "serialize_expiration")]
    refresh_token_exp: Option<CookieExpiration>,
}
// @note: We serialize the expirations, because to infer them from the cookie store we need to match
// against domain and path, which is a bit of a pain. In addition, later we may need to store the
// public key and the primary key ID here as well.
//
// We could also just store the tokens instead of the cookie store, however, it's possible that
// custom precepts may issue their own cookies, so we need to be able to handle that.

impl ClientSecrets {
    #[cfg(feature = "backend")]
    fn default_precept(
        precept_id: PreceptID,
        exp_duration: Duration,
        encoding_key: Arc<EncodingKey>,
    ) -> Self {
        Self::PreceptCookie {
            precept_id,
            exp_duration,
            encoding_key,
            cookie_header: None,
        }
    }

    #[cfg(feature = "native")]
    fn default_user(base_url: BaseUrl) -> Self {
        Self::UserCookiesNative {
            base_url,
            cookies: cookie_store::CookieStore::new(),
            access_token_exp: None,
            refresh_token_exp: None,
        }
    }

    #[cfg(feature = "native")]
    fn load_native(warnings_tx: &mpsc::Sender<anyhow::Error>, base_url: BaseUrl) -> Self {
        let context = || format!("Failed to load secrets for {}", &base_url);
        match Entry::new(KEYRING_SERVICE_NAME, &*base_url).and_then(|entry| entry.get_secret()) {
            Ok(secret) => {
                let kss = serde_json::from_slice::<KeyringSecretStorage>(&*secret)
                    .with_context(context)?;
                Self::UserCookiesNative {
                    base_url,
                    cookies: kss.cookies,
                    access_token_exp: kss.access_token_exp,
                    refresh_token_exp: kss.refresh_token_exp,
                }
            }
            Err(keyring::Error::NoEntry) => Self::default_user(base_url),
            Err(err) => {
                Err(err).context(context()).report_err(warnings_tx);
                Self::default_user(base_url)
            }
        }
    }
}

pub struct SecretProvider {
    secrets: Mutex<ClientSecrets>,
    warnings_tx: mpsc::Sender<anyhow::Error>,
}

impl PartialEq for SecretProvider {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for SecretProvider {}

impl reqwest::cookie::CookieStore for SecretProvider {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let warnings_tx = &self.warnings_tx;
        match self.secrets.lock() {
            #[cfg(feature = "backend")]
            ClientSecrets::PreceptCookie { .. } => {} // Cookies cannot be modified for precepts.
            #[cfg(feature = "native")]
            ClientSecrets::UserCookiesNative {
                ref base_url,
                ref mut cookies,
                ref mut access_token_exp,
                ref mut refresh_token_exp,
                ..
            } => {
                let parse_cookie_err = || format!("Invalid cookie header for {url}");
                let add_cookie_err = || format!("Failed to add cookie for {url}");
                let save_cookie_err = || format!("Failed to save cookies for {url}");
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
                        match cookie.name() {
                            "at" => {
                                *access_token_exp =
                                    (!cookie.is_expired()).then(|| cookie.expires.clone());
                            }
                            "rt" => {
                                *refresh_token_exp =
                                    (!cookie.is_expired()).then(|| cookie.expires.clone());
                            }
                            _ => {}
                        };
                        cookies
                            .insert(cookie.clone(), url)
                            .map(|_| ())
                            .with_context(add_cookie_err)
                            .unwrap_or_report(warnings_tx);
                        has_changes = true;
                    };
                }
                if has_changes {
                    let kss = KeyringSecretStorage {
                        cookies: mem::take(cookies), // to avoid cloning
                        access_token_exp: access_token_exp.clone(),
                        refresh_token_exp: refresh_token_exp.clone(),
                    };
                    let saved: Result<(), anyhow::Error> = try {
                        let serialized_cookies =
                            serde_json::to_vec(&kss).with_context(save_cookie_err)?;
                        Entry::new(KEYRING_SERVICE_NAME, base_url.as_str())
                            .and_then(|entry| entry.set_secret(&serialized_cookies))
                            .with_context(save_cookie_err)?
                    };
                    *cookies = kss.cookies;
                    saved.unwrap_or_report(warnings_tx);
                }
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        match self.secrets.lock() {
            #[cfg(feature = "backend")]
            ClientSecrets::PreceptCookie {
                ref precept_id,
                ref exp_duration,
                ref encoding_key,
                ref mut cookie_header,
            } => match cookie_header {
                Some(HeaderExpPair {
                    ref header,
                    ref exp,
                }) if !exp.is_expired() => Some(header.clone()),
                _ => {
                    let exp = OffsetDateTime::now_utc() + exp_duration;
                    let header: Result<HeaderValue, anyhow::Error> = try {
                        use crate::auth::middleware::{JwtClaims, JwtClaimsAccess};
                        let token = JwtClaimsAccess::new(precept_id.into(), exp)
                            .to_token(&**encoding_key)
                            .context("Failed to generate precept access token")
                            .ok_or_report(&self.warnings_tx)?;
                        let cookie_header_str =
                            cookie::Cookie::new("at", &*token.token).to_string();
                        let mut cookie_header = HeaderValue::try_from(cookie_header_str)
                            .context("Failed to construct precept access token cookie header")?;
                        cookie_header.set_sensitive(true);
                        Ok(cookie_header)
                    };
                    header.ok_or_report(&self.warnings_tx).map(|header| {
                        *cookie_header = Some(HeaderExpPair {
                            header: header.clone(),
                            exp: exp.into(),
                        });
                        header
                    })
                }
            },

            #[cfg(feature = "native")]
            ClientSecrets::UserCookiesNative { ref cookies, .. } => {
                let all_cookies = cookies
                    .get_request_values(url)
                    .map(|(name, value)| cookie::Cookie::new(name, value).to_string())
                    .join("; ");
                match all_cookies.is_empty() {
                    true => None,
                    false => HeaderValue::try_from(all_cookies)
                        .with_context(|| format!("Failed to construct cookie header for {url}"))
                        .ok_or_report(self.warnings_tx)
                        .map(|mut header_value| {
                            header_value.set_sensitive(true);
                            header_value
                        }),
                }
            }
        }
    }
}

pub enum TokenStatus {
    Valid,
    MustRefresh { url: Url },
    // MustRelogin { url: Url, primary_key_id }, @todo primary key
    MustLogin { is_expired: bool },
}

impl SecretProvider {
    #[cfg(feature = "backend")]
    pub fn new_precept(
        warnings_tx: mpsc::Sender<anyhow::Error>,
        id: PreceptID,
        token_lifetime: Duration,
        encoding_key: Arc<EncodingKey>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            secrets: Mutex::new(ClientSecrets::default_precept(id, token_lifetime, encoding_key)),
            warnings_tx,
        })
    }

    #[cfg(feature = "native")]
    pub fn new_user(
        warnings_tx: mpsc::Sender<anyhow::Error>,
        base_url: BaseUrl,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            secrets: Mutex::new(ClientSecrets::load_native(&warnings_tx, base_url)),
            warnings_tx,
        })
    }

    pub fn token_status(&self) -> TokenStatus {
        match &self.secrets.lock() {
            #[cfg(feature = "backend")]
            ClientSecrets::PreceptCookie { .. } => TokenStatus::Valid,
            #[cfg(feature = "native")]
            ClientSecrets::UserCookiesNative {
                base_url,
                access_token_exp,
                refresh_token_exp,
                ..
            } => {
                let access_expired = match access_token_exp {
                    None => true,
                    Some(exp) => exp.is_expired(),
                };
                let refresh_expired = match refresh_token_exp {
                    None => true,
                    Some(exp) => exp.is_expired(),
                };
                match (access_expired, refresh_expired) {
                    (false, _) => TokenStatus::Valid,
                    (true, false) => TokenStatus::MustRefresh {
                        url: base_url.refresh_token.clone(),
                    },
                    (true, true) => TokenStatus::MustLogin {
                        is_expired: refresh_token_exp.is_some(),
                    },
                }
            }
        }
    }

    pub async fn ensure_valid_token(&self, client: &reqwest::Client) -> precept::Result<()> {
        match self.token_status() {
            TokenStatus::Valid => Ok(()),
            TokenStatus::MustRefresh { url } => {
                #[allow(unused_variables)]
                let RefreshTokenApiResponse {
                    access_token_exp,
                    refresh_token_exp,
                } = client
                    .post(url)
                    .send()
                    .await
                    .into_precept_result_t()
                    .await?;
                #[cfg(feature = "web")]
                todo!();
                Ok(())
            }
            TokenStatus::MustLogin { is_expired } => Err(if is_expired {
                UnauthorizedError::ExpiredToken
            } else {
                UnauthorizedError::Missing
            })
            .into(),
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    prefixed_url: Arc<Url>,
    secret_provider: Arc<SecretProvider>,
}

impl HttpClient {
    pub fn new(prefixed_url: Arc<Url>, secret_provider: SecretProvider) -> Self {
        let secret_provider = Arc::new(secret_provider);
        let client = reqwest::Client::builder()
            .cookie_provider(secret_provider.clone())
            .build()
            .unwrap();
        Self {
            client,
            prefixed_url,
            secret_provider,
        }
    }

    pub async fn send<S>(&self, msg: S) -> precept::Result<S::Response>
    where
        S: precept::MessageRemoteStrategy,
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
    // @todo: consume login/refresh payload
}

impl PartialEq for HttpClient {
    fn eq(&self, other: &Self) -> bool {
        self.secret_provider == other.secret_provider && self.prefixed_url == other.prefixed_url
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
