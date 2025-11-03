use std::{sync::Arc, time::Duration};

use artilect_macro::{precept, precept_message};
use axum::{Router, extract, routing::post};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use dashmap::DashMap;
use sqlx::PgPool;
use time::UtcDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::dto::TelegramLoginStartRequest;
use crate::{
    auth::{
        User,
        dto::{
            AuthProvider, LoginAttemptStatus, LoginPollRequest, LoginPollResponse,
            TelegramLoginStartResponse,
        },
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

struct LoginAttempt {
    provider: AuthProvider,
    status: LoginAttemptStatus,
}

pub struct Resources {
    pub address_book: AddressBook,
    pub pool: PgPool,
    pub max_concurrent_login_attempts: usize,
    pub login_attempts_timeout_min: u16,
}

pub struct State {
    login_attempts_map: DashMap<u128, LoginAttempt>,
    login_attempts_expiry_queue_in: mpsc::Sender<(UtcDateTime, u128)>,
    login_attempts_expiry_queue_out: mpsc::Receiver<(UtcDateTime, u128)>,
}

#[precept(TelegramLoginStartRequest, TelegramLoginPollRequest)]
#[custom_router]
pub struct Precept {
    resources: Arc<Resources>,
    state: Arc<State>,
}

impl Precept {
    pub fn new(resources: Arc<Resources>) -> Self {
        let (tx, rx) = mpsc::channel(resources.max_concurrent_login_attempts);
        Self {
            state: Arc::new(State {
                login_attempts_map: DashMap::new(),
                login_attempts_expiry_queue_in: tx,
                login_attempts_expiry_queue_out: rx,
            }),
            resources,
        }
    }
}

impl Routable for actix::Addr<Precept> {
    fn build_router(self) -> Router {
        let mut router = axum::Router::new();
        router = TelegramLoginStartRequest::route(router);
        router = LoginPollRequest::route(router);
        router.with_state(self)
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for TelegramLoginStartRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/login/telegram", post(handle_telegram_login_start))
    }

    async fn handle(
        res: &Resources,
        state: &State,
        from: Identity,
        _: Self,
    ) -> precept::Result<TelegramLoginStartResponse> {
        match from {
            Identity::Precept { id, on_behalf_of }
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                let code = rand::random();
                state.login_attempts_map.insert(
                    code,
                    LoginAttempt {
                        provider: AuthProvider::Telegram,
                        status: LoginAttemptStatus::Pending,
                    },
                );
                state
                    .login_attempts_expiry_queue_in
                    .send((
                        UtcDateTime::now()
                            + Duration::from_mins(res.login_attempts_timeout_min.into()),
                        code,
                    ))
                    .await
                    .map_err(|e| precept::Error::Internal(anyhow::anyhow!(e)))?;
                Ok(TelegramLoginStartResponse {
                    code,
                    code_str: Uuid::from_u128(code).to_string().into(),
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
impl MessageLocalStrategy<Precept> for LoginPollRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/login/poll", post(handle_telegram_login_poll))
    }

    async fn handle(
        _res: &Resources,
        state: &State,
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        match from {
            Identity::Precept { id, on_behalf_of }
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                let successful_login_attempt =
                    state
                        .login_attempts_map
                        .remove_if(&message.code, |_, login_attempt| {
                            matches!(login_attempt.status, LoginAttemptStatus::Success { .. })
                        });
                match successful_login_attempt {
                    Some((_, login_attempt)) => Ok(login_attempt.status),
                    None => {
                        if state.login_attempts_map.contains_key(&message.code) {
                            Ok(LoginAttemptStatus::Pending)
                        } else {
                            Err(precept::Error::NotFound)
                        }
                    }
                }
            }
            _ => Err(precept::Error::Internal(anyhow::anyhow!(
                "Login start can only be called via REST API"
            ))),
        }
    }
}

#[axum::debug_handler]
async fn handle_telegram_login_poll(
    mut jar: CookieJar,
    extract::State(precept): extract::State<actix::Addr<Precept>>,
    extract::Path(attempt_id): extract::Path<Uuid>,
    axum::Json(body): axum::Json<LoginPollRequest>,
) -> precept::Result<(CookieJar, axum::Json<LoginPollResponse>)> {
    let res = send_to_self(precept, body).await?;
    match &res {
        LoginPollResponse::Pending => {}
        LoginPollResponse::Success { user } => {
            // @todo: build a JWT here
            let cookie = Cookie::build(("at", user.name.clone()))
                .http_only(true)
                .secure(true);
            jar = jar.add(cookie);
        }
    }
    Ok((jar, axum::Json(res)))
}
