use std::sync::Arc;

use axum::{
    Router,
    extract::Request,
    middleware::{Next, from_fn},
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::{OffsetDateTime, UtcDateTime, UtcOffset};
use uuid::Uuid;
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

pub enum MakeExpiration {
    FromTime(OffsetDateTime),
    FromDuration(time::Duration),
}

impl From<MakeExpiration> for OffsetDateTime {
    fn from(value: MakeExpiration) -> Self {
        match value {
            MakeExpiration::FromTime(exp) => exp,
            MakeExpiration::FromDuration(duration) => OffsetDateTime::now_utc() + duration,
        }
    }
}

pub trait JwtClaims: Clone + Serialize + DeserializeOwned + Send + Sync {
    const COOKIE_NAME: &'static str;
    type Payload: Clone + Send + Sync + 'static;
    fn new(payload: Self::Payload, exp: OffsetDateTime) -> Self;
    fn payload(&self) -> Self::Payload;
    fn to_token(&self) -> jsonwebtoken::errors::Result<GeneratedToken>;
}

// JWT Claims structure that matches your Identity enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaimsAccess {
    #[serde(flatten)]
    id: Identity,
    #[serde(with = "time::serde::timestamp")]
    exp: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    iat: OffsetDateTime,
}

impl JwtClaims for JwtClaimsAccess {
    const COOKIE_NAME: &'static str = "at";
    type Payload = Identity;

    fn new(id: Identity, exp: OffsetDateTime) -> Self {
        let iat = OffsetDateTime::now_utc();
        // let exp = iat + *crate::config::back_shared::JWT_ACCESS_LIFETIME;
        Self { id, iat, exp }
    }

    fn payload(&self) -> Self::Payload {
        self.id
    }

    fn to_token(&self) -> jsonwebtoken::errors::Result<GeneratedToken> {
        let token = encode(&jsonwebtoken::Header::default(), &self, &*JWT_ENCODING_KEY)?;
        Ok(GeneratedToken {
            token,
            exp: self.exp,
            cookie_name: Self::COOKIE_NAME,
            path: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaimsRefresh {
    ses: Uuid,
    exp: OffsetDateTime,
    iat: OffsetDateTime,
}

#[derive(Clone)]
pub struct SessionKey(pub Uuid);

impl JwtClaims for JwtClaimsRefresh {
    const COOKIE_NAME: &'static str = "rt";
    type Payload = SessionKey;

    fn new(ses: SessionKey, exp: OffsetDateTime) -> Self {
        let iat = OffsetDateTime::now_utc();
        Self {
            ses: ses.0,
            iat,
            exp,
        }
    }

    fn payload(&self) -> Self::Payload {
        SessionKey(self.ses.clone())
    }

    fn to_token(&self) -> jsonwebtoken::errors::Result<GeneratedToken> {
        let token = encode(&jsonwebtoken::Header::default(), &self, &*JWT_ENCODING_KEY)?;
        Ok(GeneratedToken {
            token,
            exp: self.exp,
            cookie_name: Self::COOKIE_NAME,
            path: Some("/auth/refresh"),
        })
    }
}

pub struct GeneratedToken {
    pub token: String,
    pub exp: OffsetDateTime,
    pub cookie_name: &'static str,
    pub path: Option<&'static str>,
}

impl GeneratedToken {
    pub fn into_cookie(self) -> Cookie<'static> {
        let mut builder = Cookie::build((self.cookie_name, self.token))
            .http_only(true)
            .secure(true)
            .expires(self.exp.to_offset(UtcOffset::UTC));
        if let Some(path) = self.path {
            builder = builder.path(path);
        }
        builder.build()
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
            req.extensions_mut().insert(token_data.claims.payload());
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
