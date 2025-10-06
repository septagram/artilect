use std::sync::Arc;

use artilect_macro::precept;
use teloxide::{
    adaptors::DefaultParseMode,
    dispatching::{Dispatcher, UpdateFilterExt},
    prelude::*,
    types::{Message, ParseMode},
    utils::command::BotCommands,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::{orchestra::AddressBook, precept::SignedMessage};

#[precept()]
pub struct Precept {
    resources: Arc<Resources>,
    bot: DefaultParseMode<Bot>,
    task_handle: Option<JoinHandle<()>>,
}

pub struct Resources {
    pub address_book: AddressBook,
    pub bot_token: Box<str>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "kebab-case", description = "Available commands:")]
enum Command {
    #[command(description = "Login to the artilect using provided code")]
    Login(String),
    #[command(description = "Get a list of available commands")]
    Help,
}

impl Precept {
    pub fn new(resources: Resources) -> Self {
        Self {
            bot: Bot::new(resources.bot_token.as_ref()).parse_mode(ParseMode::MarkdownV2),
            resources: Arc::new(resources),
            task_handle: None,
        }
    }

    async fn handle_command(
        bot: DefaultParseMode<Bot>,
        msg: Message,
        cmd: Command,
    ) -> Result<(), teloxide::RequestError> {
        match cmd {
            Command::Login(code) => {
                let _ = Uuid::parse_str(&code)
                    .map_err(|e| {
                        println!("Failed to parse UUID: {}", e);
                    })
                    .map(|uuid| {
                        println!("Successfully parsed UUID: {}", uuid);
                        uuid
                    });
            }
            Command::Help => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
        }
        Ok(())
    }

    async fn run_bot(bot: DefaultParseMode<Bot>) {
        let handler = Update::filter_message()
            .filter_command::<Command>()
            .endpoint(
                |bot: DefaultParseMode<Bot>, msg: Message, cmd: Command| async move {
                    Self::handle_command(bot, msg, cmd).await
                },
            );

        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    }
}

impl actix::Actor for Precept {
    type Context = actix::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        let bot = self.bot.clone();

        // Spawn a tokio future instead of an actix future
        self.task_handle = Some(tokio::spawn(async move {
            Self::run_bot(bot).await;
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
