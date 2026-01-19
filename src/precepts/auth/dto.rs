use std::sync::Arc;

use artilect_macro::dto;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::precept;
use crate::precept::UserIdentity;

#[dto(always, db, ui, clone, request, response)]
pub struct User {
    pub id: Uuid,
    pub name: Box<str>,
}

#[dto(always, db, ui, clone, request, response)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: AuthProvider,
    pub provider_username: Option<Box<str>>,
    pub provider_display_name: Option<Box<str>>,
}

#[dto(auth, db, clone, request, response)]
pub struct Session {
    pub id: Uuid,
    pub account_id: Uuid,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[dto(auth, eq, request, response)]
#[derive(Clone, Copy)]
// #[sqlx(type_name = "auth_provider", rename_all = "PascalCase")]
pub enum AuthProvider {
    Telegram,
    #[serde(other)]
    Unsupported,
}

impl AuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "Telegram",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[dto(auth, request)]
#[message(BotLoginStartResponse, BotLoginStartMessage)]
pub struct BotLoginStartRequest {
    pub flow_id: Box<str>,
}

#[dto(auth, response)]
pub struct BotLoginStartResponse {
    pub code: u128,
    pub code_str: Box<str>,
}

#[dto(auth, request)]
#[message(LoginPollResponse, LoginPollMessage)]
pub struct LoginPollRequest {
    pub code: u128,
}

#[dto(auth, response)]
pub enum LoginPollResponse {
    Pending,
    Success {
        user: User,
        account: Account,
        #[serde(with = "time::serde::rfc3339")]
        access_token_exp: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339::option")]
        refresh_token_exp: Option<OffsetDateTime>,
    },
}

#[dto(auth, request)]
#[message(RefreshTokenResponse, RefreshTokenMessage)]
pub struct RefreshTokenRequest {
    pub session_id: Uuid,
}

#[dto(auth, response)]
pub struct RefreshTokenResponse {
    pub user_identity: UserIdentity,
    pub access_token_lifetime: Duration,
    #[serde(with = "time::serde::rfc3339")]
    pub refresh_token_exp: OffsetDateTime,
}

#[dto(always, response)]
pub struct RefreshTokenApiResponse {
    #[serde(with = "time::serde::rfc3339")]
    pub access_token_exp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub refresh_token_exp: OffsetDateTime,
}

// #[dto(auth, request)]
// #[message(LinkResponse, LinkMessage)]
// pub struct LinkRequest {}
//
// type LinkResponse = LoginResponse;

#[dto(auth, request)]
#[message(ConfirmLoginResponse, ConfirmLoginMessage)]
pub struct ConfirmLoginRequest {
    pub code: Uuid,
    pub provider: AuthProvider,
    pub provider_user_id: Box<str>,
    pub provider_username: Option<Box<str>>,
    pub provider_display_name: Option<Box<str>>,
}

#[dto(auth, response)]
pub struct ConfirmLoginResponse {
    pub user: User,
    pub account: Account,
    pub session: Option<Session>,
}

#[dto(auth, request)]
#[message(InvalidateLoginResponse, InvalidateLoginMessage)]
pub struct InvalidateLoginRequest {
    pub code: Uuid,
}

#[dto(auth, response)]
pub struct InvalidateLoginResponse {}

// Auth provider enumeration

#[dto(auth, response)]
pub enum AuthFlowFrontend {
    Bot {
        message_template_md: Box<str>,
    },
    #[serde(other)]
    Unsupported,
}

#[dto(auth, response)]
pub struct AuthProviderInfo {
    pub id: Box<str>,
    pub name: Box<str>,
    pub icon_url: Option<Box<str>>,
    pub flow: AuthFlowFrontend,
}

#[dto(auth, request)]
#[message(ListAuthProvidersResponse, ListAuthProvidersMessage)]
pub struct ListAuthProvidersRequest {}

#[dto(auth, response)]
pub struct ListAuthProvidersResponse {
    pub providers: Arc<[AuthProviderInfo]>,
}
