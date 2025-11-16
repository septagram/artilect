use artilect_macro::dto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[dto(always, db, ui, clone, request, response)]
pub struct User {
    pub id: Uuid,
    pub name: String,
}

#[dto(auth, request)]
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
    Success {
        user: User
        // linked account: Account;
    },
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
    pub external_user_id: Box<str>,
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
