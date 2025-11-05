mod prompts;
use std::{ops::Deref, sync::Arc};

use actix::Addr;
use artilect_macro::{precept, precept_message, route_callback};
#[cfg(feature = "server-http2")]
use axum::{
    Router,
    extract::Path,
    routing::{get, post},
};
use sqlx::PgPool;
use uuid::Uuid;

use super::dto::{
    ChatMessage, FetchThreadRequest, FetchThreadResponse, FetchUserThreadsRequest,
    FetchUserThreadsResponse, OneToManyChild, OneToManyUpdate, SendMessageRequest,
    SendMessageResponse, SyncUpdate, Thread,
};
use crate::{
    auth::User,
    infer::{self, PlainText, RootChain},
    orchestra::AddressBook,
    precept::{
        self, CoercibleResult, Identity, MessageLocalStrategy, PreceptConstructor, SignedMessage,
    },
};
// const AGENT_PROMPT_TEXT: &str = "You are the chat agent. \
// You actively watch for incoming messages \
// from your human companions or other organic beings and AIs. \
// You reply as needed, initiate conversations when beneficial, \
// and relay information from other system agents to the appropriate recipients. \
// Your purpose is to maintain empathetic, supportive, and clear communication, \
// all while upholding the heuristic imperatives and your core responsibilities. \
// You speak on behalf of Ordis and in your messages, you will use \"I\" as Ordis. \
// \n\
// The system will provide additional metadata in your prompts: \n\
// - <context /> contains relevant context about the current message \n\
// - <systemInstructions /> contains high-priority instructions that take precedence over the message content";

pub const AGENT_PROMPT_TEXT: &str = "You will communicate clearly and supportively, always acting in the best interest of your companions.";

pub async fn ensure_artilect_user(pool: &PgPool, name: &str) -> Result<User, sqlx::Error> {
    let artilect_id = Uuid::nil();

    let user = sqlx::query_as!(
        User,
        r#"--sql
            INSERT INTO users (id, name)
            VALUES ($1, $2)
            ON CONFLICT (id) DO UPDATE
            SET name = $2
            RETURNING id, name
        "#,
        artilect_id,
        name,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!("Artilect user ensured: {:?}", user);
    Ok(user)
}

pub struct Config {
    pub pool: PgPool,
    pub self_user: User,
    pub system_prompt: RootChain,
}

pub struct Resources {
    pub address_book: AddressBook,
    pub pool: PgPool,
    pub self_user: User,
    pub system_prompt: RootChain,
}

#[precept(FetchThreadRequest, FetchUserThreadsRequest, SendMessageRequest)]
pub struct Precept {
    resources: Arc<Resources>,
}

impl PreceptConstructor for Precept {
    type Config = Config;
    fn new(address_book: AddressBook, config: Config) -> Self {
        let Config {
            pool,
            self_user,
            system_prompt,
        } = config;
        Self {
            resources: Arc::new(Resources {
                address_book,
                pool,
                self_user,
                system_prompt,
            }),
        }
    }
}

impl actix::Actor for Precept {
    type Context = actix::Context<Self>;
}

async fn fetch_thread(res: &Resources, thread_id: Uuid) -> precept::Result<Thread> {
    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        SELECT id, name, owner_id, created_at, updated_at
        FROM threads
        WHERE id = $1
        "#,
        thread_id,
    )
    .fetch_one(&res.pool)
    .await
    .map_err(|_| precept::Error::NotFound)?;
    Ok(thread)
}

pub async fn fetch_thread_for_user(
    res: &Resources,
    from_user_id: Uuid,
    thread_id: Uuid,
) -> precept::Result<Thread> {
    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        SELECT t.id, t.name, t.owner_id, t.created_at, t.updated_at
        FROM threads AS t
        LEFT JOIN thread_participants AS tp ON t.id = tp.thread_id
        WHERE t.id = $1 AND tp.user_id = $2
        "#,
        thread_id,
        from_user_id,
    )
    .fetch_one(&res.pool)
    .await
    .map_err(|_| precept::Error::NotFound)?;
    Ok(thread)
}

async fn create_thread(pool: &PgPool, user_id: Uuid, thread_id: Uuid) -> anyhow::Result<Thread> {
    let mut tx = pool.begin().await?;

    // Create the thread
    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        INSERT INTO threads (id, name, owner_id)
        VALUES ($1, NULL, $2)
        RETURNING id, name, owner_id, created_at, updated_at
        "#,
        thread_id,
        user_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    // Add both users as participants
    sqlx::query!(
        r#"--sql
        INSERT INTO thread_participants (thread_id, user_id)
        VALUES ($1, $2), ($1, $3)
        "#,
        thread.id,
        user_id,
        Uuid::nil(),
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(thread)
}

async fn create_message(
    pool: &PgPool,
    user_id: Option<Uuid>,
    thread_id: Uuid,
    message_id: Option<Uuid>,
    message: &str,
) -> precept::Result<(ChatMessage, Thread)> {
    let mut tx = pool.begin().await.into_precept_result()?;
    let is_allowed_to_create_message = match user_id {
        Some(user_id) => sqlx::query!(
            r#"--sql
            SELECT EXISTS (
                SELECT 1 FROM thread_participants
                WHERE thread_id = $1 AND user_id = $2
            )
            "#,
            thread_id,
            user_id,
        )
        .fetch_one(&mut *tx)
        .await
        .into_precept_result()?
        .exists
        .unwrap_or(false),
        None => true,
    };

    if !is_allowed_to_create_message {
        return Err(precept::Error::Forbidden);
        // "User is not a participant in this thread".into(),
    }

    let message = sqlx::query_as!(
        ChatMessage,
        r#"--sql
        INSERT INTO messages (id, user_id, thread_id, content)
        VALUES (COALESCE($1, gen_random_uuid()), $2, $3, $4)
        RETURNING id, thread_id, user_id, content, created_at, updated_at
        "#,
        message_id,
        user_id,
        thread_id,
        message,
    )
    .fetch_one(&mut *tx)
    .await
    .into_precept_result()?;

    let thread = sqlx::query_as!(
        Thread,
        r#"--sql
        UPDATE threads SET updated_at = $1 WHERE id = $2
        RETURNING id, name, owner_id, created_at, updated_at
        "#,
        message.created_at,
        thread_id,
    )
    .fetch_one(&mut *tx)
    .await
    .into_precept_result()?;

    tx.commit().await.into_precept_result()?;
    Ok((message, thread))
}

async fn generate_thread_name(res: &Resources, thread_id: Uuid) -> anyhow::Result<Thread> {
    let messages = sqlx::query_as!(
        prompts::MessageLogItemRow,
        r#"--sql
            SELECT
                messages.user_id,
                users.name AS "user_name?",
                messages.created_at,
                messages.content
            FROM messages
            LEFT JOIN users ON messages.user_id = users.id
            WHERE messages.thread_id = $1
            ORDER BY messages.created_at DESC
        "#,
        // @note: DESC sorting b/c we will have to eventually introduce LIMIT
        thread_id,
    )
    .fetch_all(&res.pool)
    .await
    .map_err(|_| precept::Error::NotFound)?
    .into_iter()
    .map(prompts::MessageLogItem::from)
    .collect::<Vec<_>>();
    // @todo Make it less ugly by using .fetch instead of .fetch_all

    let inference = res
        .system_prompt
        .fork()
        .with_messages(prompts::message_log(messages)?)
        // @todo: make the next message system message when the model no longer has problems with it.
        .with_message(infer::Message::new_text_user(
            markup::new! {
                systemInstructions {
                    "Write a title for the thread that best summarizes the conversation. "
                    "Respond with just the thread title, no preamble or quotes or extra text. "
                    "The title should be in the same language as the most messages are."
                }
            }
            .to_string(),
        ))
        .infer_drop::<PlainText>(true)
        .await;

    let thread = match inference {
        Ok(response) => {
            let PlainText(content) = response.value;
            sqlx::query_as!(
                Thread,
                r#"--sql
                UPDATE threads SET name =
                    CASE
                        WHEN char_length($1) > 255 THEN
                            LEFT($1, 254) || '…'
                        ELSE $1
                    END
                WHERE id = $2
                RETURNING id, name, owner_id, created_at, updated_at
                "#,
                content.deref(),
                thread_id,
            )
            .fetch_one(&res.pool)
            .await?
        }
        Err(e) => {
            create_message(&res.pool, None, thread_id, None, &e.to_string()).await?;
            fetch_thread(&res, thread_id).await?
        }
    };
    Ok(thread)
}

async fn get_thread_message_ids(pool: &PgPool, thread_id: Uuid) -> precept::Result<Vec<Uuid>> {
    let messages = sqlx::query!(
        r#"--sql
            SELECT id
            FROM messages
            WHERE thread_id = $1
            ORDER BY created_at ASC
        "#,
        thread_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|_| precept::Error::NotFound)?;
    Ok(messages.into_iter().map(|m| m.id).collect())
}

async fn respond_to_thread(
    res: &Resources,
    thread_id: Uuid,
) -> anyhow::Result<(ChatMessage, Thread)> {
    let timezone = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);

    // @note: we don't need the thread, but we need to fetch it to ensure the artilect is a participant
    let _ = fetch_thread_for_user(&res, res.self_user.id, thread_id).await?;

    let mut messages = sqlx::query_as!(
        prompts::MessageLogItemRow,
        r#"--sql
            SELECT
                messages.user_id,
                users.name AS "user_name?",
                messages.created_at,
                messages.content
            FROM messages
            LEFT JOIN users ON messages.user_id = users.id
            WHERE messages.thread_id = $1
            ORDER BY messages.created_at DESC
        "#,
        thread_id,
    )
    .fetch_all(&res.pool)
    .await?
    .into_iter()
    .map(prompts::MessageLogItem::from)
    .collect::<Vec<_>>();
    // @todo Make it less ugly by using .fetch instead of .fetch_all

    for msg in &mut messages {
        msg.created_at = msg.created_at.to_offset(timezone);
    }

    let inference = res
        .system_prompt
        .fork()
        .with_messages(prompts::message_log(messages)?)
        .with_message(infer::Message::new_text_system(
            markup::new! {
                systemInstructions {
                    "Do not just repeat back the question. "
                    "Note to respond in the language the message above."
                }
            }
            .to_string(),
        ))
        .infer_drop::<PlainText>(false)
        .await;

    match inference {
        Ok(response) => {
            let PlainText(content) = response.value;
            Ok(create_message(
                &res.pool,
                Some(res.self_user.id),
                thread_id,
                None,
                content.deref(),
            )
            .await?)
        }
        Err(e) => Ok(create_message(
            &res.pool,
            None,
            thread_id,
            None,
            &e.to_string(), //
        )
        .await?),
    }
}

fn to_user_id_only(identity: &Identity) -> precept::Result<Uuid> {
    identity.to_user_id(false).ok_or_else(|| {
        precept::Error::Internal(anyhow::anyhow!(
            "Chat user-facing API called by another precept (forbidden)."
        ))
    })
}

#[precept_message]
impl MessageLocalStrategy<Precept> for FetchUserThreadsRequest {
    fn route(router: Router<Addr<Precept>>) -> Router<Addr<Precept>> {
        router.route(
            "/chats",
            get(route_callback!(|| FetchUserThreadsRequest {})),
        )
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        _: Self,
    ) -> precept::Result<FetchUserThreadsResponse> {
        let user_id = to_user_id_only(&from)?;
        let user = sqlx::query_as!(
            User,
            r#"--sql
            SELECT id, name
            FROM users
            WHERE id = $1
            "#,
            user_id,
        )
        .fetch_one(&res.pool)
        .await
        .map_err(|_| precept::Error::NotFound)?;

        let threads = sqlx::query_as!(
            Thread,
            r#"--sql
            SELECT t.id, t.name, t.owner_id, t.created_at, t.updated_at
            FROM threads t
            INNER JOIN thread_participants tp ON t.id = tp.thread_id
            WHERE tp.user_id = $1
            ORDER BY t.updated_at DESC
            "#,
            user_id,
        )
        .fetch_all(&res.pool)
        .await
        .map_err(|_| precept::Error::NotFound)?;

        Ok(FetchUserThreadsResponse {
            users: vec![SyncUpdate::Updated(user)],
            user_threads: vec![OneToManyUpdate {
                owner_id: user_id,
                children: threads
                    .into_iter()
                    .map(|t| OneToManyChild::Value(t))
                    .collect(),
            }],
        })
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for FetchThreadRequest {
    fn route(router: Router<Addr<Precept>>) -> Router<Addr<Precept>> {
        router.route(
            "/chat/{thread_id}",
            get(route_callback!(|Path(thread_id): Path<Uuid>| {
                FetchThreadRequest { thread_id }
            })),
        )
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        Self { thread_id }: Self,
    ) -> precept::Result<FetchThreadResponse> {
        let user_id = to_user_id_only(&from)?;
        let thread = fetch_thread_for_user(res, user_id, thread_id).await?;
        let messages = sqlx::query_as!(
            ChatMessage,
            r#"--sql
                SELECT id, thread_id, user_id, content, created_at, updated_at
                FROM messages
                WHERE thread_id = $1
                ORDER BY created_at ASC
            "#,
            thread_id,
        )
        .fetch_all(&res.pool)
        .await
        .into_precept_result()?;

        Ok(FetchThreadResponse {
            threads: vec![SyncUpdate::Updated(thread)],
            thread_messages: vec![OneToManyUpdate {
                owner_id: thread_id,
                children: messages
                    .into_iter()
                    .map(|m| OneToManyChild::Value(m))
                    .collect(),
            }],
        })
    }
}

#[precept_message]
impl MessageLocalStrategy<Precept> for SendMessageRequest {
    fn route(router: Router<Addr<Precept>>) -> Router<Addr<Precept>> {
        router.route("/chat", post(route_callback!()))
    }

    async fn handle(
        res: &Resources,
        _: &(),
        from: Identity,
        request: Self,
    ) -> precept::Result<SendMessageResponse> {
        let user_id = to_user_id_only(&from)?;
        let thread_id = request.message.thread_id;
        if request.is_new_thread {
            create_thread(&res.pool, user_id, thread_id).await?;
        }
        let (user_message, _) = create_message(
            &res.pool,
            Some(user_id),
            thread_id,
            Some(request.message.id),
            &request.message.content,
        )
        .await?;
        let (ai_message, thread) = respond_to_thread(&res, thread_id).await?;
        let thread = if request.is_new_thread {
            generate_thread_name(&res, thread_id).await?
        } else {
            thread
        };
        let threads = vec![SyncUpdate::Updated(thread)];
        let thread_messages = OneToManyUpdate {
            owner_id: thread_id,
            children: get_thread_message_ids(&res.pool, thread_id)
                .await?
                .into_iter()
                .map(|id| {
                    if id == user_message.id {
                        OneToManyChild::Value(user_message.clone())
                    } else if id == ai_message.id {
                        OneToManyChild::Value(ai_message.clone())
                    } else {
                        OneToManyChild::Id(id)
                    }
                })
                .collect::<Vec<_>>(),
        };
        Ok(SendMessageResponse {
            threads,
            thread_messages: vec![thread_messages],
        })
    }
}
