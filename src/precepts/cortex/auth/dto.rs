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
    pub attempt_id: Uuid,
    pub code: Box<str>,
}

#[dto(auth, request)]
#[message(TelegramLoginPollResponse, TelegramLoginPollMessage)]
pub struct TelegramLoginPollRequest {
    pub attempt_id: Uuid,
}

#[dto(auth, response)]
pub enum TelegramLoginPollResponse {
    Pending,
    Success {
        user: User
        // linked account: Account;
    },
}

// #[dto(auth, request)]
// #[message(LinkResponse, LinkMessage)]
// pub struct LinkRequest {}
//
// type LinkResponse = LoginResponse;

#[dto(auth, request)]
#[message(ConfirmLoginResponse, ConfirmLoginMessage)]
pub struct ConfirmLoginRequest {
    pub attempt_id: Uuid,
    pub provider: AuthProvider,
    pub external_user_id: Box<str>,
}

#[dto(auth, response)]
pub struct ConfirmLoginResponse {}
