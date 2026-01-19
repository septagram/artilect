use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::precept::{Identity, Error};
pub use super::middleware::SessionKey;

impl<S> FromRequestParts<S> for SessionKey
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<SessionKey>()
            .map(|session_key| session_key.clone())
            .ok_or_else(|| anyhow::anyhow!("Session key not found in request extensions. Check if middleware is installed.").into())
    }
}

impl<S> FromRequestParts<S> for Identity
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Identity>()
            .map(|session_key| session_key.clone())
            .ok_or_else(|| anyhow::anyhow!("Identity not found in request extensions. Check if middleware is installed.").into())
    }
}
