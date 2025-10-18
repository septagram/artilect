use std::sync::Arc;
use std::time::Duration;
use actix::Addr;
use axum::Router;
use axum::routing::post;
use mini_moka::sync::Cache;
use sqlx::PgPool;
use uuid::Uuid;
use artilect_macro::{precept, precept_message, route_callback};
use crate::orchestra::AddressBook;
use crate::{precept, precept::MessageLocalStrategy};
use crate::precept::Identity;
use super::dto::{ConfirmLoginRequest, LoginRequest, LoginResponse};

pub struct Resources {
    pub address_book: AddressBook,
    pub pool: PgPool,
    pub max_concurrent_login_attempts: u32,
    pub login_attempts_timeout_min: u16,
}

pub struct State {
    login_attempts: Cache<Uuid, Uuid>,
}

#[precept(LoginRequest, ConfirmLoginRequest)]
#[custom_router]
pub struct Precept {
    resources: Arc<Resources>,
    state: Arc<State>,
}

impl Precept {
    pub fn new(resources: Arc<Resources>) -> Self {
        Self {
            state: Cache::builder()
                .max_capacity(resources.max_concurrent_login_attempts.into())
                .time_to_live(Duration::from_mins(resources.login_attempts_timeout_min.into()))
                .build()
                .into(),
            resources,
        }
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for LoginRequest {
    fn route(router: Router<Addr<Precept>>) -> Router<Addr<Precept>> {
        router.route("/login", post(route_callback!()))
    }

    async fn handle(res: &Resources, state: &State, from: Identity, _: Self) -> precept::Result<LoginResponse> {
        // state.login_attempts.insert()
        todo!()
    }
}