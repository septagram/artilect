use chat_dto::User;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub self_user: User,
    pub system_prompt: String,
} 