use chat_dto::User;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub self_user: User,
    pub user_id: Uuid,
    pub system_prompt: String,
} 