use std::{sync::Arc, time::Duration};

use artilect_macro::{precept, precept_message, route_callback};
use axum::{Router, extract, routing::post};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use dashmap::DashMap;
use sqlx::PgPool;
use time::UtcDateTime;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::dto::TelegramLoginStartRequest;
use crate::{
    auth::{
        User,
        dto::{
            AuthProvider, ConfirmLoginRequest, InvalidateLoginRequest, LoginAttemptStatus,
            LoginPollRequest, LoginPollResponse, TelegramLoginStartResponse,
        },
        middleware::RouterAuth,
    },
    orchestra::AddressBook,
    precept,
    precept::{
        ActixResult, Identity, MessageLocalStrategy, PreceptConstructor, PreceptID, Routable,
        SignedMessage,
    },
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

pub struct Config {
    pub pool: PgPool,
    pub max_concurrent_login_attempts: usize,
    pub login_attempts_timeout_min: u16,
}

pub struct Resources {
    pub address_book: AddressBook,
    pub pool: PgPool,
    pub login_attempts_map: DashMap<u128, LoginAttempt>,
    pub login_attempts_expiry_queue: mpsc::Sender<(UtcDateTime, u128)>,
    pub login_attempts_timeout: Duration,
}

#[precept(TelegramLoginStartRequest, TelegramLoginPollRequest)]
#[custom_router]
pub struct Precept {
    resources: Arc<Resources>,
    stop_signal: Option<oneshot::Sender<()>>,
}

impl actix::Actor for Precept {
    type Context = actix::Context<Self>;

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        self.stop_signal
            .take()
            .expect("Stop signal not present on shutdown")
            .send(());
    }
}

impl PreceptConstructor for Precept {
    type Config = Config;
    fn new(address_book: AddressBook, config: Config) -> Self {
        let Config {
            pool,
            max_concurrent_login_attempts,
            login_attempts_timeout_min,
        } = config;
        let (expire_tx, mut expire_rx) = mpsc::channel(max_concurrent_login_attempts);
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let resources = Arc::new(Resources {
            address_book,
            pool,
            login_attempts_map: DashMap::new(),
            login_attempts_expiry_queue: expire_tx,
            login_attempts_timeout: Duration::from_mins(login_attempts_timeout_min.into()),
        });

        let res = resources.clone();
        tokio::spawn(async move {
            loop {
                let next_expiry = tokio::select! {
                    biased;
                    _ = &mut stop_rx => break,
                    next_expiry = expire_rx.recv() => next_expiry,
                };
                let (expiry, code) = next_expiry.expect("Login attempts expiry queue closed");
                if expiry > UtcDateTime::now() {
                    if !res.login_attempts_map.contains_key(&code) {
                        continue;
                    };
                    tokio::select! {
                        biased;
                        _ = &mut stop_rx => break,
                        _ = tokio::time::sleep((expiry - UtcDateTime::now()).unsigned_abs()) => {}
                    }
                }
                let _ = res.login_attempts_map.remove(&code);
            }
        });

        Self {
            resources,
            stop_signal: Some(stop_tx),
        }
    }

    fn id(_config: &Self::Config) -> PreceptID {
        PreceptID::Auth
    }
}

impl Routable for actix::Addr<Precept> {
    fn build_router(self) -> Router {
        let mut router = axum::Router::new();
        router = ConfirmLoginRequest::route(router);
        router = InvalidateLoginRequest::route(router);
        router = router.require_access_token();
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
        _: &(),
        from: Identity,
        _: Self,
    ) -> precept::Result<TelegramLoginStartResponse> {
        match from {
            Identity::Precept { id, on_behalf_of }
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                let code = rand::random();
                res.login_attempts_map.insert(
                    code,
                    LoginAttempt {
                        provider: AuthProvider::Telegram,
                        status: LoginAttemptStatus::Pending,
                    },
                );
                res.login_attempts_expiry_queue
                    .send((UtcDateTime::now() + res.login_attempts_timeout, code))
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
        router.route("/login/poll", post(handle_login_poll))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        match from {
            Identity::Precept { id, on_behalf_of }
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                let successful_login_attempt =
                    res.login_attempts_map
                        .remove_if(&message.code, |_, login_attempt| {
                            matches!(login_attempt.status, LoginAttemptStatus::Success { .. })
                        });
                match successful_login_attempt {
                    Some((_, login_attempt)) => Ok(login_attempt.status),
                    None => {
                        if res.login_attempts_map.contains_key(&message.code) {
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

async fn handle_login_poll(
    mut jar: CookieJar,
    extract::State(precept): extract::State<actix::Addr<Precept>>,
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

#[precept_message]
impl MessageLocalStrategy<Precept> for ConfirmLoginRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/svc/attempt/confirm", post(route_callback!()))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        match identity_to_auth_provider(from) {
            Some(provider) => {
                let code_u128 = message.code.as_u128();
                if let Some(mut entry) = res.login_attempts_map.get_mut(&code_u128) {
                    if entry.provider != provider {
                        return Err(precept::Error::Forbidden);
                    };
                    // @todo: fill in the actual user details
                    let user = User {
                        id: Uuid::new_v4(),
                        name: message.external_user_id.clone().into(),
                    };
                    entry.status = LoginAttemptStatus::Success { user: user.clone() };
                    Ok(crate::auth::dto::ConfirmLoginResponse { user })
                } else {
                    Err(precept::Error::NotFound)
                }
            }
            None => Err(precept::Error::Forbidden),
        }
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for InvalidateLoginRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/svc/attempt/invalidate", post(route_callback!()))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        match identity_to_auth_provider(from) {
            Some(provider) => {
                let code_u128 = message.code.as_u128();
                let had_entry = res
                    .login_attempts_map
                    .remove_if(&code_u128, |_, login_attempt| {
                        matches!(login_attempt.status, LoginAttemptStatus::Success { .. })
                    })
                    .is_some();
                match had_entry {
                    true => Ok(crate::auth::dto::InvalidateLoginResponse {}),
                    false => Err(precept::Error::NotFound),
                }
            }
            None => Err(precept::Error::Forbidden),
        }
    }
}

fn identity_to_auth_provider(id: Identity) -> Option<AuthProvider> {
    match id {
        Identity::Precept { id: precept_id, .. } => match precept_id {
            #[cfg(any(feature = "telegram-in", feature = "telegram-out"))]
            PreceptID::Telegram => Some(AuthProvider::Telegram),
            _ => None,
        },
        _ => None,
    }
}
