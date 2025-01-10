use chat_dto::User;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub infer_url: String,
    pub model: String,
    pub pool: PgPool,
    pub self_user: User,
    pub user_id: Uuid,
} 