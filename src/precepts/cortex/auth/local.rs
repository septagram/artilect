use std::{sync::Arc, time::Duration};

use artilect_macro::{precept, precept_message};
use axum::{
    Router, extract,
    routing::{get, post},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use mini_moka::sync::Cache;
use sqlx::PgPool;
use uuid::Uuid;

use super::dto::TelegramLoginStartRequest;
use crate::{
    auth::{
        User,
        dto::{TelegramLoginPollRequest, TelegramLoginPollResponse, TelegramLoginStartResponse},
    },
    orchestra::AddressBook,
    precept,
    precept::{ActixResult, Identity, MessageLocalStrategy, PreceptID, Routable, SignedMessage},
};

async fn send_to_self<T>(precept: actix::Addr<Precept>, request: T) -> precept::Result<T::Response>
where
    T: MessageLocalStrategy<Precept>,
{
    precept
        .send(SignedMessage {
            from: Identity::Precept {
                id: PreceptID::Auth,
                on_behalf_of: None,
            },
            data: request,
        })
        .await
        .map_actix_error()
}

pub struct Resources {
    pub address_book: AddressBook,
    pub pool: PgPool,
    pub max_concurrent_login_attempts: u32,
    pub login_attempts_timeout_min: u16,
}

pub struct State {
    login_attempts: Cache<Uuid, Uuid>,
}

#[precept(TelegramLoginStartRequest, TelegramLoginPollRequest)]
#[custom_router]
pub struct Precept {
    resources: Arc<Resources>,
    state: Arc<State>,
}

impl Precept {
    pub fn new(resources: Arc<Resources>) -> Self {
        Self {
            state: Arc::new(State {
                login_attempts: Cache::builder()
                    .max_capacity(resources.max_concurrent_login_attempts.into())
                    .time_to_live(Duration::from_mins(
                        resources.login_attempts_timeout_min.into(),
                    ))
                    .build(),
            }),
            resources,
        }
    }
}

impl Routable for actix::Addr<Precept> {
    fn build_router(self) -> Router {
        let mut router = axum::Router::new();
        router = TelegramLoginStartRequest::route(router);
        router = TelegramLoginPollRequest::route(router);
        router.with_state(self)
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for TelegramLoginStartRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/login/telegram", post(handle_telegram_login_start))
    }

    async fn handle(
        _res: &Resources,
        state: &State,
        from: Identity,
        _: Self,
    ) -> precept::Result<TelegramLoginStartResponse> {
        match from {
            Identity::Precept { id, on_behalf_of }
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                let attempt_id = Uuid::new_v4();
                let code: Uuid = Uuid::new_v4();
                state.login_attempts.insert(Uuid::new_v4(), code);
                Ok(TelegramLoginStartResponse {
                    attempt_id,
                    code: code.to_string().into(),
                })
            }
            _ => Err(precept::Error::Internal(anyhow::anyhow!(
                "Login start can only be called via REST API"
            ))),
        }
    }
}

async fn handle_telegram_login_start(
    extract::State(precept): extract::State<actix::Addr<Precept>>,
) -> precept::Result<axum::Json<TelegramLoginStartResponse>> {
    send_to_self(precept, TelegramLoginStartRequest {})
        .await
        .map(|response| axum::Json(response))
}

#[precept_message]
impl MessageLocalStrategy<Precept> for TelegramLoginPollRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route(
            "/login/telegram/{attempt_id}",
            get(handle_telegram_login_poll),
        )
    }

    async fn handle(
        _res: &Resources,
        state: &State,
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        todo!()
    }
}

#[axum::debug_handler]
async fn handle_telegram_login_poll(
    mut jar: CookieJar,
    extract::State(precept): extract::State<actix::Addr<Precept>>,
    extract::Path(attempt_id): extract::Path<Uuid>,
) -> precept::Result<(CookieJar, axum::Json<TelegramLoginPollResponse>)> {
    let res = send_to_self(precept, TelegramLoginPollRequest { attempt_id }).await?;
    match &res {
        TelegramLoginPollResponse::Pending => {}
        TelegramLoginPollResponse::Success { user } => {
            // @todo: build a JWT here
            let cookie = Cookie::build(("at", user.name.clone()))
                .http_only(true)
                .secure(true);
            jar = jar.add(cookie);
        }
    }
    Ok((jar, axum::Json(res)))
}
