use std::{convert::Infallible, sync::Arc};

use actix::{Addr, AsyncContext};
use anyhow::anyhow;
use artilect_macro::{dto, precept};
use axum::Router;
use teloxide::{
    Bot,
    adaptors::DefaultParseMode,
    dispatching::{Dispatcher, HandlerExt, UpdateFilterExt},
    prelude::{Requester, RequesterExt},
    types as tg,
    utils::{command::BotCommands, markdown::escape},
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::{
    auth,
    auth::dto::{AuthProvider, ConfirmLoginRequest, InvalidateLoginRequest},
    orchestra::AddressBook,
    precept,
    precept::{
        Identity, MessageLocalStrategy, PreceptConstructor, PreceptID, SignedMessage,
        local::ActixResult,
    },
};

#[precept(CommandReceived)]
pub struct Precept {
    resources: Arc<Resources>,
    task_handle: Option<JoinHandle<()>>,
}

pub struct Config {
    pub bot_token: Box<str>,
}

pub struct Resources {
    bot: DefaultParseMode<Bot>,
    address_book: AddressBook,
}

type CommandReceivedResponse = ();

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "kebab-case", description = "Available commands:")]
enum Command {
    #[command(description = "Login to the artilect using provided code")]
    Login(String),
    #[command(description = "Get a list of available commands")]
    Help,
}

impl PreceptConstructor for Precept {
    type Config = Config;
    fn new(address_book: AddressBook, config: Config) -> Self {
        let bot = Bot::new(config.bot_token.as_ref()).parse_mode(tg::ParseMode::MarkdownV2);
        Self {
            resources: Arc::new(Resources { address_book, bot }),
            task_handle: None,
        }
    }

    fn id(_config: &Self::Config) -> PreceptID {
        PreceptID::Telegram
    }
}

async fn run_bot(addr: actix::Addr<Precept>, bot: DefaultParseMode<Bot>) {
    let handler = tg::Update::filter_message()
        .filter_command::<Command>()
        .endpoint(
            move |bot: DefaultParseMode<Bot>, msg: tg::Message, cmd: Command| /* -> impl Future<Output = Result<(), Infallible>> */{
                let addr = addr.clone();
                let chat_id = msg.chat.id;
                async move {
                    let res = addr.send(SignedMessage {
                        from: Identity::Precept {
                            id: PreceptID::Telegram,
                            on_behalf_of: None,
                        },
                        data: CommandReceived {
                            message: msg,
                            command: cmd,
                        },
                    })
                        .await
                        .map_actix_error();
                    let res_handled = if let Err(err) = res {
                        bot.send_message(chat_id, escape(err.into_telegram_response().as_str())).await.err()
                    } else {
                        None
                    };
                    if let Some(err) = res_handled {
                        tracing::error!("Error sending error reply: {:?}", err);
                    };
                    Ok::<(), Infallible>(())
                }
            },
        );

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

impl actix::Actor for Precept {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        let bot = self.resources.bot.clone();

        // Spawn a tokio future instead of an actix future
        self.task_handle = Some(tokio::spawn(async move {
            run_bot(addr, bot).await;
        }));
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> actix::Running {
        // Unspawn the future when the agent stops
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        actix::Running::Stop
    }
}

#[dto(telegram)]
#[message(CommandReceivedResponse, CommandReceivedMessage)]
struct CommandReceived {
    message: tg::Message,
    command: Command,
}

enum PrivateChatWithUser {
    Some {
        id: u64,
        username: Option<Box<str>>,
        display_name: Option<Box<str>>,
    },
    NoUser,
    NoPrivateChat,
}

impl From<tg::Message> for PrivateChatWithUser {
    fn from(msg: tg::Message) -> Self {
        match (msg.from, &msg.chat.kind) {
            (Some(user), tg::ChatKind::Private(_)) => {
                let mut display_name = user.first_name;
                if let Some(last_name) = user.last_name {
                    if !display_name.is_empty() {
                        display_name.push(' ');
                    }
                    display_name.push_str(&last_name);
                }
                let display_name = if display_name.is_empty() {
                    None
                } else {
                    Some(Box::from(display_name))
                };
                Self::Some {
                    id: user.id.0,
                    username: user.username.map(|s| s.into()),
                    display_name,
                }
            }
            (None, _) => Self::NoUser,
            (_, tg::ChatKind::Public(_)) => Self::NoPrivateChat,
        }
    }
}

impl MessageLocalStrategy<Precept> for CommandReceived {
    fn route(router: Router<Addr<Precept>>) -> Router<Addr<Precept>> {
        router
    }
    async fn handle(
        resources: &Resources,
        _state: &(),
        from: Identity,
        Self { message, command }: Self,
    ) -> precept::Result<Self::Response> {
        let chat_id = message.chat.id;
        match command {
            Command::Login(code) => {
                let response = match (
                    PrivateChatWithUser::from(message),
                    Uuid::parse_str(code.as_str()),
                ) {
                    (
                        PrivateChatWithUser::Some {
                            id: user_id,
                            username,
                            display_name,
                        },
                        Ok(code),
                    ) => {
                        let user = resources
                            .address_book
                            .auth
                            .send(ConfirmLoginRequest {
                                code,
                                provider: AuthProvider::Telegram,
                                provider_user_id: format!("{}", user_id).into(),
                                provider_username: username,
                                provider_display_name: display_name,
                            })
                            .await?
                            .user;
                        format!(
                            "You've been logged in successfully as {}. You can now go back to the login page in your browser.",
                            user.name
                        )
                    }
                    (PrivateChatWithUser::NoUser, _) => {
                        String::from("I can only login users, I'm afraid.")
                    }
                    (PrivateChatWithUser::NoPrivateChat, Ok(code)) => {
                        let invalidate_result = resources
                            .address_book
                            .auth
                            .send(InvalidateLoginRequest { code })
                            .await;
                        String::from(
                            "You should not login from a public/group chat. I'm afraid I had to invalidate the login code.",
                        )
                    }
                    (_, Err(_)) => String::from("Invalid code format."),
                };
                resources
                    .bot
                    .send_message(chat_id, escape(response.as_str()))
                    .await
                    .map_err(|e| anyhow!(e))?;
            }
            Command::Help => {
                resources
                    .bot
                    .send_message(chat_id, Command::descriptions().to_string())
                    .await
                    .map_err(|e| anyhow!(e))?;
            }
        }
        Ok(())
    }
}

trait IntoTelegramResponse {
    fn into_telegram_response(self) -> String;
}

impl IntoTelegramResponse for precept::Error {
    fn into_telegram_response(self) -> String {
        if matches!(self, precept::Error::Internal(_)) {
            String::from("Internal error")
        } else {
            self.to_string()
        }
    }
}
