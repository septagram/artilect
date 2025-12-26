use std::{collections::HashMap, sync::Arc};

use actix::fut::err;
use anyhow::{Context, anyhow};
use artilect_macro::{dto, precept, precept_message, route_callback};
use axum::{
    Extension, Router, extract,
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, Expiration},
};
use dashmap::DashMap;
use indoc::formatdoc;
use jsonwebtoken::{DecodingKey, EncodingKey};
use sqlx::{PgPool, postgres::types::PgInterval};
use time::{Duration, OffsetDateTime, PrimitiveDateTime, UtcDateTime, UtcOffset};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::dto::BotLoginStartRequest;
use crate::{
    auth::{
        SessionKey, User,
        dto::{
            Account, AuthFlowFrontend, AuthProvider, AuthProviderInfo, BotLoginStartResponse,
            ConfirmLoginRequest, InvalidateLoginRequest, ListAuthProvidersRequest,
            ListAuthProvidersResponse, LoginPollRequest, LoginPollResponse,
            RefreshTokenApiResponse, RefreshTokenRequest, RefreshTokenResponse, Session,
        },
        middleware::{
            JwtClaims, JwtClaimsAccess, JwtClaimsRefresh, MakeExpiration, RouterAuth,
            get_jwt_key_pair,
        },
    },
    orchestra::AddressBook,
    precept,
    precept::{
        ActixResult, Identity, IntoPreceptResult, MessageLocalStrategy, PreceptConstructor,
        PreceptID, Routable, SignedMessage, UnauthorizedError, UserIdentity,
    },
};

#[dto(auth, request)]
#[message(LoginAttemptStatusResponse, LoginAttemptStatusMessage)]
pub struct LoginAttemptStatusQuery {
    pub code: u128,
}

// Internal only
#[dto(auth, response)]
pub enum LoginAttemptStatus {
    Pending,
    Success {
        user: User,
        account: Account,
        session: Option<Session>,
    },
}

pub enum LoginAttemptStatusResponse {
    Pending,
    Success {
        user: User,
        account: Account,
        session: Option<Session>,
        access_token_lifetime: Duration,
    },
}

pub struct AuthProviderConfig {
    pub name: Box<str>,
    pub icon_url: Option<Box<str>>,
    pub flow: AuthFlowBackend,
}

pub enum AuthFlowBackend {
    // /// Cryptographic private key authentication
    // PrivateKey,
    /// Bot-based authentication (e.g., Telegram bot)
    Bot {
        precept_id: PreceptID,
        message_template_md: Box<str>,
    },
    // /// Authentication via another Artilect instance
    // Artilect,
    // /// Standard OAuth 2.0 flow
    // OAuth,
    // /// Traditional email/password authentication
    // EmailPassword,
}

impl AuthFlowBackend {
    fn id(&self) -> Box<str> {
        match self {
            Self::Bot { precept_id, .. } => match precept_id {
                PreceptID::Telegram => "telegram".into(),
                _ => panic!("Unsupported precept ID for bot auth flow"),
            },
        }
    }

    fn to_auth_provider_info(&self) -> AuthProviderInfo {
        let id = self.id();
        match self {
            AuthFlowBackend::Bot {
                precept_id,
                message_template_md,
            } => match precept_id {
                PreceptID::Telegram => AuthProviderInfo {
                    id,
                    name: "Telegram".into(),
                    icon_url: Some("/assets/icon-tg.svg".into()),
                    flow: AuthFlowFrontend::Bot {
                        message_template_md: message_template_md.clone(),
                    },
                },
                _ => panic!("Unsupported precept ID for bot auth flow"),
            },
        }
    }

    pub fn default_flows(tg_bot_name: String) -> Vec<AuthFlowBackend> {
        vec![AuthFlowBackend::Bot {
            precept_id: PreceptID::Telegram,
            message_template_md: formatdoc! {"
                Paste the following command into the chat with [@{tg_bot_name}](https://t.me/{tg_bot_name}):
                ```
                /login {{code_str}}
                ```
            "}.into(),
        }]
    }
}

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
    pub max_concurrent_login_attempts: usize,
    pub login_attempts_timeout_min: u16,
    pub auth_providers: Vec<AuthFlowBackend>,
    pub rest: StoredConfig,
}

pub struct StoredConfig {
    pub pool: PgPool,
    pub instance_id: Box<str>,
    pub access_token_lifetime: Duration,
    pub refresh_token_lifetime: Option<Duration>,
}

pub struct Resources {
    address_book: AddressBook,
    login_attempts_map: DashMap<u128, LoginAttempt>,
    login_attempts_expiry_queue: mpsc::Sender<(UtcDateTime, u128)>,
    login_attempts_timeout: Duration,
    auth_providers: HashMap<Box<str>, AuthFlowBackend>,
    auth_providers_info: Arc<[AuthProviderInfo]>,
    encoding_key: EncodingKey,
    decoding_key: Arc<DecodingKey>,
    rest: StoredConfig,
}

#[precept(
    TelegramLoginStartRequest,
    TelegramLoginPollRequest,
    ListAuthProvidersRequest
)]
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
    fn new(address_book: AddressBook, config: Config) -> Result<Self, anyhow::Error> {
        let Config {
            max_concurrent_login_attempts,
            login_attempts_timeout_min,
            auth_providers: auth_providers_config,
            rest,
        } = config;
        let (expire_tx, mut expire_rx) = mpsc::channel(max_concurrent_login_attempts);
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let mut auth_providers_info = Vec::new();
        let mut auth_providers = HashMap::new();
        for auth_provider in auth_providers_config {
            auth_providers_info.push(auth_provider.to_auth_provider_info());
            auth_providers.insert(auth_provider.id(), auth_provider);
        }
        let (encoding_key, decoding_key) = get_jwt_key_pair(&*rest.instance_id)?;
        let resources = Arc::new(Resources {
            address_book,
            login_attempts_map: DashMap::new(),
            login_attempts_expiry_queue: expire_tx,
            login_attempts_timeout: Duration::minutes(login_attempts_timeout_min.into()),
            auth_providers,
            auth_providers_info: auth_providers_info.into(),
            encoding_key,
            decoding_key: Arc::new(decoding_key),
            rest,
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

        Ok(Self {
            resources,
            stop_signal: Some(stop_tx),
        })
    }

    fn id(_config: &Self::Config) -> PreceptID {
        PreceptID::Auth
    }
}

impl Routable for actix::Addr<Precept> {
    fn build_router(self) -> Router {
        let mut service_routes = Router::new();
        service_routes = ConfirmLoginRequest::route(service_routes);
        service_routes = InvalidateLoginRequest::route(service_routes);
        service_routes = service_routes.require_access_token();
        let mut refresh_routes = Router::new();
        refresh_routes = RefreshTokenRequest::route(refresh_routes);
        refresh_routes = refresh_routes.require_refresh_token();
        let mut entry_routes = Router::new();
        entry_routes = BotLoginStartRequest::route(entry_routes);
        entry_routes = ListAuthProvidersRequest::route(entry_routes);
        entry_routes = entry_routes.route("/login/poll", post(handle_login_poll));
        entry_routes
            .merge(refresh_routes)
            .merge(service_routes)
            .with_state(self)
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for BotLoginStartRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/login/bot/{flow_id}", post(handle_bot_login_start))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        Self { flow_id }: Self,
    ) -> precept::Result<BotLoginStartResponse> {
        let AuthFlowBackend::Bot { precept_id, .. } = res
            .auth_providers
            .get(&flow_id)
            .ok_or(precept::Error::NotFound)?;
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
                Ok(BotLoginStartResponse {
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

async fn handle_bot_login_start(
    extract::State(precept): extract::State<actix::Addr<Precept>>,
    extract::Path(flow_id): extract::Path<Box<str>>,
) -> precept::Result<axum::Json<BotLoginStartResponse>> {
    send_to_self(precept, BotLoginStartRequest { flow_id })
        .await
        .map(|response| axum::Json(response))
}

#[precept_message]
impl MessageLocalStrategy<Precept> for LoginAttemptStatusQuery {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        unreachable!("Login attempt status request is handled by the login poll endpoint")
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
                    Some((_, login_attempt)) => Ok(match login_attempt.status {
                        LoginAttemptStatus::Pending => LoginAttemptStatusResponse::Pending,
                        LoginAttemptStatus::Success {
                            user,
                            account,
                            session,
                        } => LoginAttemptStatusResponse::Success {
                            user,
                            account,
                            session,
                            access_token_lifetime: res.rest.access_token_lifetime,
                        },
                    }),
                    None => {
                        if res.login_attempts_map.contains_key(&message.code) {
                            Ok(LoginAttemptStatusResponse::Pending)
                        } else {
                            Err(precept::Error::NotFound)
                        }
                    }
                }
            }
            _ => Err(precept::Error::Internal(anyhow::anyhow!(
                "Login poll can only be called via REST API"
            ))),
        }
    }
}

async fn handle_login_poll(
    extract::State(precept): extract::State<actix::Addr<Precept>>,
    axum::Json(body): axum::Json<LoginAttemptStatusQuery>,
) -> precept::Result<(CookieJar, axum::Json<LoginPollResponse>)> {
    let mut jar = CookieJar::new();
    let res = match send_to_self(precept, body).await? {
        LoginAttemptStatusResponse::Pending => LoginPollResponse::Pending,
        LoginAttemptStatusResponse::Success {
            user,
            account,
            session,
            access_token_lifetime,
        } => {
            let identity = Identity::User(UserIdentity {
                user_id: user.id,
                account_id: account.id,
            });
            let access_token = JwtClaimsAccess::new(
                identity,
                MakeExpiration::FromDuration(access_token_lifetime).into(),
            )
            .to_token()
            .context("Failed to create access token")?;
            let access_token_exp = access_token.exp;
            jar = jar.add(access_token.into_cookie());
            let refresh_token_exp = match session {
                Some(session) => {
                    let refresh_token = JwtClaimsRefresh::new(
                        SessionKey(session.id),
                        MakeExpiration::FromTime(session.expires_at).into(),
                    )
                    .to_token()
                    .context("Failed to create refresh token")?;
                    let exp = refresh_token.exp;
                    jar = jar.add(refresh_token.into_cookie());
                    Some(exp)
                }
                None => None,
            };
            LoginPollResponse::Success {
                user,
                account,
                access_token_exp,
                refresh_token_exp,
            }
        }
    };
    Ok((jar, axum::Json(res)))
}

impl MessageLocalStrategy<Precept> for RefreshTokenRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/refresh", post(handle_refresh_token))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        message: Self,
    ) -> precept::Result<Self::Response> {
        struct SessionDataRow {
            user_id: Option<Uuid>,
            account_id: Option<Uuid>,
            expires_at: Option<PrimitiveDateTime>,
        }

        impl SessionDataRow {
            fn into_response(
                self,
                access_token_lifetime: Duration,
            ) -> Result<RefreshTokenResponse, precept::Error> {
                match (self.user_id, self.account_id, self.expires_at) {
                    (Some(user_id), Some(account_id), Some(expires_at)) => {
                        Ok(RefreshTokenResponse {
                            user_identity: UserIdentity {
                                user_id,
                                account_id,
                            },
                            access_token_lifetime,
                            refresh_token_exp: expires_at.as_utc().to_offset(UtcOffset::UTC),
                        })
                    }
                    _ => Err(precept::Error::Internal(anyhow::anyhow!(
                        "Invalid refresh_session() return values"
                    ))),
                }
            }
        }

        match (res.rest.refresh_token_lifetime, from) {
            (Some(lifetime), Identity::Precept { id, on_behalf_of })
                if id == PreceptID::Auth && on_behalf_of.is_none() =>
            {
                sqlx::query_as!(
                    SessionDataRow,
                    r#"--sql
                        SELECT user_id, account_id, expires_at
                        FROM refresh_session($1, $2)
                    "#,
                    message.session_id,
                    PgInterval::try_from(lifetime)
                        .map_err(|_| anyhow::anyhow!("Failed to convert access token lifetime"))?,
                )
                .fetch_one(&res.rest.pool)
                .await
                .map_err(|err| match err {
                    sqlx::Error::RowNotFound => {
                        precept::Error::Unauthorized(UnauthorizedError::InvalidSession)
                    }
                    other_err => precept::Error::Internal(anyhow::anyhow!(other_err)),
                })?
                .into_response(lifetime)
            }
            (None, _) => Err(precept::Error::Forbidden),
            _ => Err(precept::Error::Internal(anyhow::anyhow!(
                "Token refresh can only be called via REST API"
            ))),
        }
    }
}

async fn handle_refresh_token(
    session_key: Option<Extension<SessionKey>>,
    extract::State(precept): extract::State<actix::Addr<Precept>>,
) -> precept::Result<(CookieJar, axum::Json<RefreshTokenApiResponse>)> {
    let session_key = session_key.ok_or(UnauthorizedError::Missing)?.0;
    let RefreshTokenResponse {
        user_identity,
        access_token_lifetime,
        refresh_token_exp,
    } = send_to_self(
        precept,
        RefreshTokenRequest {
            session_id: session_key.0,
        },
    )
    .await?;
    let access_token_exp = OffsetDateTime::now_utc() + access_token_lifetime;
    let mut jar = CookieJar::new();
    jar = jar.add(
        JwtClaimsAccess::new(Identity::User(user_identity), access_token_exp)
            .to_token()
            .context("Failed to create access token")?
            .into_cookie(),
    );
    jar = jar.add(
        JwtClaimsRefresh::new(session_key, refresh_token_exp)
            .to_token()
            .context("Failed to create refresh token")?
            .into_cookie(),
    );
    Ok((
        jar,
        axum::Json(RefreshTokenApiResponse {
            access_token_exp,
            refresh_token_exp,
        }),
    ))
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
        struct UserRow {
            user_id: Option<Uuid>,
            user_name: Option<String>,
            account_id: Option<Uuid>,
            session_id: Option<Uuid>,
            session_created_at: Option<OffsetDateTime>,
            session_expires_at: Option<OffsetDateTime>,
        }

        impl UserRow {
            fn try_decompose(
                self,
                provider: AuthProvider,
                provider_username: Option<Box<str>>,
                provider_display_name: Option<Box<str>>,
            ) -> precept::Result<(User, Account, Session)> {
                if let Some(user_id) = self.user_id
                    && let Some(user_name) = self.user_name
                    && let Some(account_id) = self.account_id
                    && let Some(session_id) = self.session_id
                    && let Some(session_created_at) = self.session_created_at
                    && let Some(session_expires_at) = self.session_expires_at
                {
                    Ok((
                        User {
                            id: user_id,
                            name: user_name.into(),
                        },
                        Account {
                            id: account_id,
                            user_id,
                            provider,
                            provider_username,
                            provider_display_name,
                        },
                        Session {
                            id: session_id,
                            account_id,
                            created_at: session_created_at,
                            expires_at: session_expires_at,
                        },
                    ))
                } else {
                    Err(precept::Error::Internal(anyhow!("Invalid user row")))
                }
            }
        }

        match identity_to_auth_provider(from) {
            Some(provider) => {
                let code_u128 = message.code.as_u128();
                if let Some(mut entry) = res.login_attempts_map.get_mut(&code_u128) {
                    if entry.provider != provider || message.provider != provider {
                        return Err(precept::Error::Forbidden);
                    };
                    let (user, account, session) = sqlx::query_as!(
                        UserRow,
                        r#"--sql
                            SELECT user_id, user_name, account_id, session_id, session_created_at, session_expires_at
                            FROM get_user_from_login($1, $2, $3, $4, $5)
                        "#,
                        provider as AuthProvider,
                        &message.provider_user_id,
                        message.provider_username.as_deref(),
                        message.provider_display_name.as_deref(),
                        PgInterval::try_from(res.refresh_token_lifetime.unwrap_or(Duration::ZERO))
                            .map_err(|_| anyhow::anyhow!("Failed to convert access token lifetime"))?,
                    )
                        .fetch_one(&res.pool)
                        .await
                        .into_precept_result()?
                        .try_decompose(provider, message.provider_username, message.provider_display_name)?;
                    let session = res.refresh_token_lifetime.and(Some(session));
                    entry.status = LoginAttemptStatus::Success {
                        user: user.clone(),
                        account: account.clone(),
                        session: session.clone(),
                    };
                    Ok(crate::auth::dto::ConfirmLoginResponse {
                        user,
                        account,
                        session,
                    })
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
                        login_attempt.provider == provider
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
    // @todo: make the list of auth providers configurable
    match id {
        Identity::Precept { id: precept_id, .. } => match precept_id {
            PreceptID::Telegram => Some(AuthProvider::Telegram),
            _ => None,
        },
        _ => None,
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for ListAuthProvidersRequest {
    fn route(router: Router<actix::Addr<Precept>>) -> Router<actix::Addr<Precept>> {
        router.route("/providers", get(handle_list_auth_providers))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        _from: Identity,
        _: Self,
    ) -> precept::Result<ListAuthProvidersResponse> {
        Ok(ListAuthProvidersResponse {
            providers: res.auth_providers_info.clone(),
        })
    }
}

async fn handle_list_auth_providers(
    extract::State(precept): extract::State<actix::Addr<Precept>>,
) -> precept::Result<axum::Json<ListAuthProvidersResponse>> {
    send_to_self(precept, ListAuthProvidersRequest {})
        .await
        .map(|response| axum::Json(response))
}
