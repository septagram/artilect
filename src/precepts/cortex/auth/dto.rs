use artilect_macro::dto;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[dto(auth, request)]
pub enum ChatProvider {
    Telegram,
}

#[dto(auth, request)]
#[message(LoginResponse, LoginMessage)]
pub struct LoginRequest {
    pub user_id: Option<Uuid>,
}

#[dto(auth, response)]
pub struct LoginResponse {
    pub attempt_id: Uuid,
}

#[dto(auth, request)]
#[message(ConfirmLoginResponse, ConfirmLoginMessage)]
pub struct ConfirmLoginRequest {
    pub attempt_id: Uuid,
    pub provider: ChatProvider,
    pub external_user_id: Box<str>,
}

#[dto(auth, response)]
pub struct ConfirmLoginResponse {}
