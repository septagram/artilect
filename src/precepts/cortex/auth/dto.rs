use std::sync::Arc;

use artilect_macro::dto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::precept;

#[dto(always, db, ui, clone, request, response)]
pub struct User {
    pub id: Uuid,
    pub name: Box<str>,
}

#[dto(auth, eq, request)]
#[derive(sqlx::Type)]
#[sqlx(type_name = "auth_provider", rename_all = "PascalCase")]
pub enum AuthProvider {
    Telegram,
}

#[dto(auth, request)]
#[message(TelegramLoginStartResponse, TelegramLoginStartMessage)]
pub struct TelegramLoginStartRequest {}

#[dto(auth, response)]
pub struct TelegramLoginStartResponse {
    pub code: u128,
    pub code_str: Box<str>,
}

#[dto(auth, request)]
#[message(LoginPollResponse, TelegramLoginPollMessage)]
pub struct LoginPollRequest {
    pub code: u128,
}

#[dto(auth, response)]
pub enum LoginAttemptStatus {
    Pending,
    Success { user: User }, // @todo +linked account: Account;
}

pub type LoginPollResponse = LoginAttemptStatus;

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
