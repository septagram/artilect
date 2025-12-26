use std::sync::Arc;

use anyhow::Context;
use axum::{
    Router,
    extract::{Request, State},
    middleware::{Next, from_fn_with_state},
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::{OffsetDateTime, UtcOffset};
use uuid::Uuid;

use crate::{
    precept,
    precept::{Identity, UnauthorizedError},
};

pub const KEYRING_SERVICE_NAME: &str = "artilect-cortex";
pub const JWT_SECRET_NAME: &str = "jwt-secret";

pub fn get_secret(instance_id: &str, secret_name: &str) -> Result<Box<[u8]>, anyhow::Error> {
    let context = |reason| || format!("{reason} JWT secret for instance {}", instance_id);
    let entry = Entry::new(
        format!("{}.{}", instance_id, KEYRING_SERVICE_NAME).as_str(),
        secret_name,
    )
    .with_context(context("Invalid keyring name for"))?;
    match entry.get_secret() {
        Ok(secret) => Ok(Arc::from(secret)),
        Err(err) => match err {
            keyring::Error::NoEntry => {
                // Generate cryptographically secure random secret
                let mut secret = [0u8; 32];
                rand::fill(&mut secret);

                // Store it in keyring
                entry
                    .set_secret(&secret)
                    .with_context(context("Failed to store"))?;

                secret.into()
            }
            _ => Err(err).context(context("Failed to retrieve")()),
        },
    }
}

pub fn get_jwt_key_pair(instance_id: &str) -> Result<(EncodingKey, DecodingKey), anyhow::Error> {
    let secret = get_secret(instance_id, JWT_SECRET_NAME)?;
    Ok((
        EncodingKey::from_secret(&secret),
        DecodingKey::from_secret(&secret),
    ))
}

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
    fn to_token(&self, encoding_key: &EncodingKey) -> jsonwebtoken::errors::Result<GeneratedToken>;
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

    fn to_token(&self, encoding_key: &EncodingKey) -> jsonwebtoken::errors::Result<GeneratedToken> {
        let token = encode(&jsonwebtoken::Header::default(), &self, encoding_key)?;
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

    fn to_token(&self, encoding_key: &EncodingKey) -> jsonwebtoken::errors::Result<GeneratedToken> {
        let token = encode(&jsonwebtoken::Header::default(), &self, encoding_key)?;
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
    State(decoding_key): State<Arc<DecodingKey>>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> precept::Result<Response> {
    // Get the JWT cookie using CookieJar
    match jar.get(Claims::COOKIE_NAME) {
        Some(cookie) => {
            let token = cookie.value();

            // Decode and validate JWT
            let token_data =
                decode::<Claims>(token, &*decoding_key, &TOKEN_VALIDATION).map_err(|e| match e
                    .kind()
                {
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
    fn require_access_token(self, decoding_key: Arc<DecodingKey>) -> Self;
    fn require_refresh_token(self, decoding_key: Arc<DecodingKey>) -> Self;
}

impl<T> RouterAuth for Router<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn require_access_token(self, decoding_key: Arc<DecodingKey>) -> Self {
        self.layer(from_fn_with_state(decoding_key, jwt::<JwtClaimsAccess>))
    }

    fn require_refresh_token(self, decoding_key: Arc<DecodingKey>) -> Self {
        self.layer(from_fn_with_state(decoding_key, jwt::<JwtClaimsRefresh>))
    }
}
