use actix::{Actor, Context};
use chat_dto::User;
use sqlx::PgPool;

#[derive(Clone)]
pub struct ChatService {
    pool: PgPool,
    self_user: User,
    system_prompt: String,
} 

impl Actor for ChatService {
    type Context = Context<Self>;
}

impl ChatService {
    pub fn new(pool: PgPool, self_user: User, system_prompt: String) -> Self {
        Self { pool, self_user, system_prompt }
    }
}
