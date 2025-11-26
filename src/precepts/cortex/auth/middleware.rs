use std::sync::Arc;

use axum::{
    Router,
    extract::Request,
    middleware::{Next, from_fn},
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::UtcDateTime;

use crate::{
    precept,
    precept::{Identity, UnauthorizedError},
};

pub const KEYRING_SERVICE_NAME: &str = "artilect-cortex";
pub const ARTILECT_INSTANCE_ID: &str = "artilect"; // To support running multiple artilects, make dynamic.
pub const JWT_SECRET_NAME: &str = "jwt-secret";

static JWT_SECRET: Lazy<Box<[u8]>> = Lazy::new(|| {
    let entry = Entry::new(
        format!("{}.{}", ARTILECT_INSTANCE_ID, KEYRING_SERVICE_NAME).as_str(),
        JWT_SECRET_NAME,
    )
    .expect("Invalid keyring name for JWT secret");
    match entry.get_secret() {
        Ok(secret) => Box::from(secret),
        Err(err) => match err {
            keyring::Error::NoEntry => {
                // Generate cryptographically secure random secret
                let mut secret = [0u8; 32];
                rand::fill(&mut secret);

                // Store it in keyring
                entry
                    .set_secret(&secret)
                    .expect("Failed to store JWT secret in keyring");

                secret.into()
            }
            _ => panic!("Failed to get secret from keyring: {}", err),
        },
    }
});

static JWT_ENCODING_KEY: Lazy<EncodingKey> =
    Lazy::new(|| EncodingKey::from_secret(JWT_SECRET.as_ref()));
static JWT_DECODING_KEY: Lazy<DecodingKey> =
    Lazy::new(|| DecodingKey::from_secret(JWT_SECRET.as_ref()));

static TOKEN_VALIDATION: Lazy<Validation> = Lazy::new(|| Validation::new(Algorithm::HS256));

trait JwtClaims: Clone + DeserializeOwned + Send + Sync {
    const COOKIE_NAME: &'static str;
    type Payload: Clone + Send + Sync + 'static;
    fn to_payload(&self) -> Self::Payload;
}

// JWT Claims structure that matches your Identity enum
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaimsAccess {
    #[serde(flatten)]
    id: Identity,
    exp: i64,
    iat: i64,
}

impl JwtClaims for JwtClaimsAccess {
    const COOKIE_NAME: &'static str = "at";
    type Payload = Identity;
    fn to_payload(&self) -> Self::Payload {
        self.id.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaimsRefresh {
    ses: Arc<str>,
    exp: i64,
    iat: i64,
}

#[derive(Clone)]
pub struct SessionKey(pub Arc<str>);

impl JwtClaims for JwtClaimsRefresh {
    const COOKIE_NAME: &'static str = "rt";
    type Payload = SessionKey;
    fn to_payload(&self) -> Self::Payload {
        SessionKey(self.ses.clone())
    }
}

// Middleware to extract and validate JWT from cookie
async fn jwt<Claims: JwtClaims>(
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> precept::Result<Response> {
    // Get the JWT cookie using CookieJar
    match jar.get(Claims::COOKIE_NAME) {
        Some(cookie) => {
            let token = cookie.value();

            // Decode and validate JWT
            let token_data = decode::<Claims>(token, &*JWT_DECODING_KEY, &TOKEN_VALIDATION)
                .map_err(|e| match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        UnauthorizedError::ExpiredToken
                    }
                    _ => UnauthorizedError::InvalidToken,
                })?;

            // Insert identity into request extensions
            req.extensions_mut().insert(token_data.claims.to_payload());
            Ok(next.run(req).await)
        }
        None => Err(UnauthorizedError::Missing.into()),
    }
}

pub trait RouterAuth {
    fn require_access_token(self) -> Self;
    fn require_refresh_token(self) -> Self;
}

impl<T> RouterAuth for Router<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn require_access_token(self) -> Self {
        self.layer(from_fn(jwt::<JwtClaimsAccess>))
    }

    fn require_refresh_token(self) -> Self {
        self.layer(from_fn(jwt::<JwtClaimsRefresh>))
    }
}

pub enum AccessTokenType {
    User,
    Precept,
}

pub struct GeneratedToken {
    pub token: Box<str>,
    pub exp: i64,
}

pub fn make_access_token(id: Identity, token_type: AccessTokenType) -> precept::Result<GeneratedToken> {
    let now = UtcDateTime::now();
    let iat = now.unix_timestamp();
    let exp = match token_type {
        AccessTokenType::User => {
            (now + *crate::config::back_shared::JWT_ACCESS_LIFETIME).unix_timestamp()
        }
        AccessTokenType::Precept => i64::MAX,
    };
    let claims = JwtClaimsAccess { id, iat, exp };
    match encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &*JWT_ENCODING_KEY,
    ) {
        Ok(token) => Ok(GeneratedToken {
            token: token.into(),
            exp,
        }),
        Err(error) => Err(anyhow::anyhow!(error).into()),
    }
}

#[cfg(feature = "client-http2")]
pub mod http_client {
    use std::sync::Arc;
    use reqwest::cookie::{CookieStore, Jar};
    use url::Url;
    use keyring::Entry;
    use crate::precept::{Identity, PreceptID};
    use super::{make_access_token, AccessTokenType, KEYRING_SERVICE_NAME, ARTILECT_INSTANCE_ID};

    #[derive(Clone, PartialEq)]
    pub enum CookieStorage {
        Precept { id: PreceptID },
        User { base_url: Arc<str> },
    }

    impl CookieStorage {
        fn get_or_create_token(&self) -> String {
            match self {
                CookieStorage::Precept { id } => {
                    // Generate token for precept
                    let identity = Identity::Precept {
                        id: id.clone(),
                        on_behalf_of: None,
                    };
                    make_access_token(identity, AccessTokenType::Precept)
                        .expect("Failed to create precept token")
                        .token
                        .to_string()
                }
                CookieStorage::User { base_url } => {
                    // Retrieve token from keyring
                    let key = format!("user-token-{}", base_url);
                    let entry = Entry::new(
                        &format!("{}.{}", ARTILECT_INSTANCE_ID, KEYRING_SERVICE_NAME),
                        &key,
                    )
                    .expect("Invalid keyring entry");

                    entry
                        .get_password()
                        .unwrap_or_else(|_| String::new())
                }
            }
        }

        fn set_token(&self, token: &str) {
            if let CookieStorage::User { base_url } = self {
                let key = format!("user-token-{}", base_url);
                let entry = Entry::new(
                    &format!("{}.{}", ARTILECT_INSTANCE_ID, KEYRING_SERVICE_NAME),
                    &key,
                )
                .expect("Invalid keyring entry");

                let _ = entry.set_password(token);
            }
        }
    }

    #[derive(Clone)]
    pub struct HttpClient {
        client: reqwest::Client,
        cookie_storage: CookieStorage,
    }

    impl HttpClient {
        pub fn new(cookie_storage: CookieStorage) -> Self {
            let jar = Arc::new(Jar::default());

            // Pre-populate with token if available
            let token = cookie_storage.get_or_create_token();
            if !token.is_empty() {
                // Add cookie for any URL - reqwest will handle the domain matching
                if let Ok(url) = Url::parse("http://localhost") {
                    jar.add_cookie_str(&format!("at={}", token), &url);
                }
            }

            let client = reqwest::Client::builder()
                .cookie_provider(jar)
                .build()
                .expect("Failed to create HTTP client");

            Self {
                client,
                cookie_storage,
            }
        }

        pub fn client(&self) -> &reqwest::Client {
            &self.client
        }
    }

    impl PartialEq for HttpClient {
        fn eq(&self, other: &Self) -> bool {
            self.cookie_storage == other.cookie_storage
        }
    }
}
