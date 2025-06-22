use indoc::formatdoc;
use ouroboros::self_referencing;
use uuid::Uuid;
use std::sync::Arc;

pub mod config;
mod error;
pub use error::InferError;
mod openai;
use openai::{ApiError, OpenAIMessage, OpenAIContentPart};
mod parsing;
mod util;

pub use parsing::{FromLlmReply, ParseError, PlainText, WithReasoning, YesNoReply};

const AGENT_PROMPT_TEXT: &str = "You are the inference agent. \
You are responsible for assisting other agents by solving \
various isolated problems.";

pub struct Client {
    id: Uuid,
}

impl Client {
    pub fn new() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl MessageRole {
    pub fn into_role_str(self, convert_system_to_user: bool) -> &'static str {
        match self {
            Self::System if convert_system_to_user => openai::ROLE_USER,
            Self::System => openai::ROLE_SYSTEM,
            Self::User => openai::ROLE_USER,
            Self::Assistant => openai::ROLE_ASSISTANT,
        }
    }
}

// push content blocks instead, e.g.
// pub enum ContentBlock {
//     NewMessage(MessageRole),
//     Text(Box<str>),
//     ImageUrl { url: String },
//     ImageFile { data: Vec<u8> },
//     // ... other content types
// }

pub struct Message {
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn new_text(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentBlock::Text(text.into().into())],
        }
    }

    pub fn new_text_user(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: vec![ContentBlock::Text(text.into().into())],
        }
    }

    pub fn new_text_assistant(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text(text.into().into())],
        }
    }

    pub fn new_text_system(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: vec![ContentBlock::Text(text.into().into())],
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(Box<str>),
}

pub struct ChainLink {
    pub item: ChainItem,
    pub prev: Option<Arc<ChainLink>>,
}

#[derive(Debug, Clone)]
pub enum ChainItem {
    NewMessage(MessageRole),
    ContentBlock(ContentBlock),
}

pub struct Chain<'a> {
    id: Uuid,
    client: &'a Client,
    tail: Option<Arc<ChainLink>>,
    item_count: usize,
    message_count: usize,
}

impl<'a> Clone for Chain<'a> {
    fn clone(&self) -> Self {
        Self {
            id: Uuid::new_v4(),
            client: self.client,
            tail: self.tail.clone(),
            item_count: self.item_count,
            message_count: self.message_count,
        }
    }
}

impl<'a> Chain<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self {
            id: Uuid::new_v4(),
            client,
            tail: None,
            item_count: 0,
            message_count: 0,
        }
    }

    pub fn set_client(&mut self, client: &'a Client) {
        self.client = client;
    }

    pub fn with_client(mut self, client: &'a Client) -> Self {
        self.client = client;
        self
    }

    pub fn push_message(&mut self, Message { role, content }: Message) {
        self.push_item(ChainItem::NewMessage(role));
        for block in content {
            self.push_item(ChainItem::ContentBlock(block));
        }
    }

    pub fn push_item(&mut self, item: ChainItem) {
        if self.tail.is_none() && !matches!(item, ChainItem::NewMessage(_)) {
            panic!("Cannot push content to empty chain, must push NewMessage first");
        }
        self.item_count += 1;
        if matches!(item, ChainItem::NewMessage(_)) {
            self.message_count += 1;
        }
        self.tail = Some(Arc::from(ChainLink {
            prev: self.tail.clone(),
            item,
        }));
    }

    pub fn with_message(mut self, message: Message) -> Self {
        self.push_message(message);
        self
    }

    pub fn with_item(mut self, item: ChainItem) -> Self {
        self.push_item(item);
        self
    }

    pub fn extend_messages<I>(&mut self, messages: I)
    where
        I: IntoIterator<Item = Message>,
    {
        for message in messages {
            self.push_message(message);
        }
    }

    pub fn with_messages<I>(mut self, messages: I) -> Self
    where
        I: IntoIterator<Item = Message>,
    {
        self.extend_messages(messages);
        self
    }

    pub fn extend_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = ChainItem>,
    {
        for item in items {
            self.push_item(item);
        }
    }

    pub fn with_items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = ChainItem>,
    {
        self.extend_items(items);
        self
    }

    async fn infer_str(&self, toggle_reasoning: Option<bool>) -> Result<Box<str>, InferError> {
        let mut messages = self.as_openai_messages();
        if *config::MODEL_HAS_TOGGLEABLE_REASONING
            && let Some(toggle_reasoning) = toggle_reasoning
            && let append_str = if toggle_reasoning {
                config::THINK_ON_POSTFIX.clone()
            } else {
                config::THINK_OFF_POSTFIX.clone()
            }
            && !append_str.is_empty()
            && let Some(last_message) = messages.last_mut()
            && let Some(last_content) = last_message.content.last_mut()
        {
            if let OpenAIContentPart::Text { text } = last_content {
                text.push_str(&append_str);
            } else {
                last_message.content.push(OpenAIContentPart::Text {
                    text: append_str.to_string(),
                });
            }
        }

        tracing::info!("Prompt:\n{}", util::wrap_and_indent_yaml(&messages));

        match openai::openai_request(&messages, &config::DEFAULT_MODEL, &config::INFER_URL).await {
            Ok(response) => {
                tracing::info!("Response:\n{}", util::wrap_and_indent_yaml(&response));
                Ok(response)
            },
            Err(error) => match error {
                ApiError::ErrorResponse(error_text) => Err(
                    match Box::pin(is_context_length_error(self.client, error_text.as_str())).await {
                        Ok(true) => InferError::ContextLengthError(Arc::from(error_text)),
                        Ok(false) => ApiError::ErrorResponse(error_text.clone()).into(),
                        Err(second_error) => {
                            eprintln!(
                                "Warning: Error from is_context_length_error: {:?}",
                                second_error
                            );
                            ApiError::ErrorResponse(error_text.clone()).into()
                        }
                    },
                ),
                _ => return Err(error.into()),
            },
        }
    }

    async fn infer_and_parse<T: FromLlmReply>(
        &self,
        toggle_reasoning: Option<bool>,
    ) -> Result<T, InferError> {
        Ok(T::from_reply(&self.infer_str(toggle_reasoning).await?)?)
    }

    async fn infer_and_extract<T: FromLlmReply>(
        &self,
        with_reasoning: bool,
    ) -> Result<(WithReasoning<T>, Box<str>), InferError> {
        Ok(if *config::MODEL_HAS_REASONING {
            let response = self.infer_str(Some(with_reasoning)).await?;
            let (value, _, value_str) = WithReasoning::<T>::parse(&response)?;
            (value, value_str.into())
        } else if with_reasoning {
            let reasoning_response = self
                .clone()
                .with_item(
                    ChainItem::ContentBlock(ContentBlock::Text(
                        crate::system_instructions! {
                            "Do not answer just yet, just think out loud, step by step, how it should be answered."
                        }
                    ))
                )
                .infer_str(None).await?;
            let value_response = self
                .clone()
                .with_item(ChainItem::NewMessage(MessageRole::Assistant))
                .with_item(
                    ChainItem::ContentBlock(
                        ContentBlock::Text(format!("\n<think>{reasoning_response}</think>\n\n").into())
                    )
                )
                .infer_str(None)
                .await?;
            let value = WithReasoning::<T> {
                reasoning: Some(reasoning_response),
                value: T::from_reply(&value_response)?,
            };
            (value, value_response)
        } else {
            let response = self.infer_str(None).await?;
            let value = WithReasoning::<T> {
                reasoning: None,
                value: T::from_reply(&response)?,
            };
            (value, response)
        })
    }

    pub async fn infer_drop<T: FromLlmReply>(
        self,
        with_reasoning: bool,
    ) -> Result<WithReasoning<T>, InferError> {
        let (value, _) = self.infer_and_extract(with_reasoning).await?;
        Ok(value)
    }

    pub async fn infer_keep<T: FromLlmReply>(
        &self,
        with_reasoning: bool,
    ) -> Result<WithReasoning<T>, InferError> {
        let (value, _) = self.infer_and_extract(with_reasoning).await?;
        Ok(value)
    }

    pub async fn infer_push<T: FromLlmReply>(
        &mut self,
        with_reasoning: bool,
    ) -> Result<WithReasoning<T>, InferError> {
        let (value, value_str) = self.infer_and_extract(with_reasoning).await?;
        // @note: backend already knows about these items, so we don't need to push them normally
        self.tail = Some(Arc::from(ChainLink {
            prev: self.tail.clone(),
            item: ChainItem::NewMessage(MessageRole::Assistant),
        }));
        self.tail = Some(Arc::from(ChainLink {
            prev: self.tail.clone(),
            item: ChainItem::ContentBlock(ContentBlock::Text(value_str)),
        }));
        Ok(value)
    }

    pub fn as_openai_messages(&self) -> Vec<OpenAIMessage> {
        let do_convert_system_to_user = !*config::MODEL_USE_SYSTEM_PROMPT;
        let mut chain_items = Vec::with_capacity(self.item_count);
        let mut prev = self.tail.clone();
        while let Some(cur_item) = prev {
            chain_items.push(cur_item.item.clone());
            prev = cur_item.prev.clone();
        }
        let mut chain_items = chain_items.into_iter().rev();
        let first_item = chain_items.next();
        match first_item {
            None => return Vec::new(),
            Some(first_item) => {
                match first_item {
                    ChainItem::NewMessage(role) => {
                        let mut messages = Vec::with_capacity(self.message_count);
                        let mut cur_message = OpenAIMessage {
                            role: role.into_role_str(do_convert_system_to_user),
                            content: Vec::new(),
                        };
                        for item in chain_items {
                            match item {
                                ChainItem::NewMessage(role) => {
                                    messages.push(std::mem::take(&mut cur_message));
                                    cur_message.role = role.into_role_str(do_convert_system_to_user);
                                },
                                ChainItem::ContentBlock(block) => {
                                    match block {
                                        ContentBlock::Text(text) => {
                                            let last_content = cur_message.content.last_mut();
                                            if let Some(last_content) = last_content 
                                                && let OpenAIContentPart::Text { text: last_text } = last_content
                                                && !last_text.is_empty()
                                            {
                                                last_text.push_str(text.as_ref());
                                            } else {
                                                cur_message.content.push(OpenAIContentPart::Text {
                                                    text: text.to_string(),
                                                });
                                            }
                                        },
                                    }
                                },
                            }
                        }
                        messages.push(cur_message);
                        messages
                    },
                    _ => panic!("First item must be NewMessage"),
                }
            }
        }
    }
}

#[self_referencing]
pub struct RootChain {
    client: Client,
    #[borrows(client)]
    #[covariant]
    chain: Chain<'this>,
}

impl RootChain {
    pub fn from_message(client: Client, message: Message) -> Self {
        RootChainBuilder {
            client,
            chain_builder: |client| {
                Chain::new(client).with_message(message)
            },
        }.build()
    }

    pub fn from_messages(client: Client, messages: impl IntoIterator<Item = Message>) -> Self {
        RootChainBuilder {
            client,
            chain_builder: |client| {
                Chain::new(client).with_messages(messages)
            },
        }.build()
    }

    pub fn fork(&self) -> Chain {
        self.with_chain(|chain| chain.clone())
    }
}

pub fn get_artilect_name() -> String {
    let name = std::env::var("NAME")
        .expect("NAME must be set")
        .trim()
        .to_string();

    if name.is_empty() {
        panic!("NAME cannot be empty");
    }

    name
}

pub async fn is_context_length_error(client: &Client, error: &str) -> Result<bool, InferError> {
    let quoted_error = format!("\"{}\"", error.replace('\\', "\\\\").replace('"', "\\\""));

    Ok(Chain::new(client)
        .with_message(
            Message::new_text_user(
                formatdoc! {"
                    The following is an error message from OpenAI: {quoted_error}.
                    Is this an error about context length?

                    With no preamble, respond with a JSON object in the following format: {{
                        \"answer\": true if this is a context length error, false otherwise
                    }}
                "}
            )
        )
        .infer_drop::<YesNoReply>(false)
        .await?
        .value
        .into())
}

#[cfg(test)]
mod tests {
    use artilect_macro::FromLlmReplyArrayItem;
    use once_cell::sync::Lazy;
    use parsing::{FromLlmReplyArray, FromLlmReplyArrayItem};
    use serde::Deserialize;
    use dotenvy::dotenv;

    use super::*;

    static CLIENT: Lazy<Client> = Lazy::new(|| {
        dotenv().ok();
        Client { id: Uuid::new_v4() }
    });

    mod top_level_array_parsing {
        use super::*;
        #[tokio::test]
        async fn parses_top_level_array_of_strings() {
            let result = Chain::new(&CLIENT).with_message(Message {
                role: MessageRole::User,
                content: formatdoc! {"
                    # Instructions

                    Break down the following text into a list of lines, return as JSON array of strings:

                    And you, my father, there on the sad height,
                    Curse, bless, me now with your fierce tears, I pray.
                    Do not go gentle into that good night.
                    Rage, rage against the dying of the light.

                    # Format Instructions

                    With no preamble, respond with a JSON array of strings.
                "}.into(),
            }).infer_drop::<Vec<Box<str>>>(false)
                .await
                .map(|lines| {
                    lines
                        .value
                        .into_iter()
                        .map(|line| {
                            line.chars()
                                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                                .collect::<String>()
                                .to_lowercase()
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .into()
                        })
                        .collect::<Vec<Box<str>>>()
                });
            assert!(result.is_ok());
            let expected: Vec<Box<str>> = vec![
                "and you my father there on the sad height".into(),
                "curse bless me now with your fierce tears i pray".into(),
                "do not go gentle into that good night".into(),
                "rage rage against the dying of the light".into(),
            ];
            assert_eq!(result.unwrap(), expected);
        }
        #[tokio::test]
        async fn parses_top_level_array_of_objects() {
            #[derive(Debug, Deserialize, FromLlmReplyArrayItem, PartialEq)]
            struct SpaceObject {
                name: Box<str>,
                mass: f64,
                habitable: bool,
            }

            let result = Chain::new(&CLIENT).with_message(Message {
                role: MessageRole::User,
                content: formatdoc! {"
                    # Instructions

                    Here are some interesting celestial objects to consider:

                    The Sun is our home star and the center of our solar system. It has a mass of 1.0 solar masses by definition.
                    Proxima Centauri b is an exoplanet orbiting the nearest star to our Sun. It's about 1.17 times Earth's mass (0.0000035 solar masses) and lies in the habitable zone.
                    Betelgeuse is a red supergiant star with approximately 19 solar masses. It's far too hot to be habitable.

                    Parse this information into structured data.

                    # Format Instructions

                    Respond with a JSON array of objects in the following format:
                    {{
                        \"name\": the name of the celestial object
                        \"mass\": the mass of the celestial object in solar masses
                        \"habitable\": true if the celestial object is habitable, false otherwise
                    }}
                "}.into(),
            }).infer_drop::<Vec<SpaceObject>>(true).await;
            assert!(result.is_ok());

            let objects = result.unwrap().value;
            assert_eq!(objects.len(), 3);

            // Verify structure is parsed correctly by checking first object
            assert!(objects[0].name.len() > 0);
            assert!(objects[0].mass > 0.0);
            assert!(matches!(objects[0].habitable, true | false));
            assert!(objects[0].name.to_lowercase().contains("sun"));
            assert_eq!(objects[0].mass, 1.0);
            // Habitable is in the eye of the beholder, as far as LLMs go, apparently :P

            assert!(objects[1].name.to_lowercase().contains("centauri"));
            assert_eq!(objects[1].mass, 0.0000035);
            assert!(objects[1].habitable);

            assert!(objects[2].name.to_lowercase().contains("betelgeuse"));
            assert_eq!(objects[2].mass, 19.0);
            assert!(!objects[2].habitable);
        }
    }

    mod context_length_error {
        use super::*;

        #[tokio::test]
        async fn detects_context_length_error() {
            let error = "Trying to keep the first 5406 tokens when context the overflows. \
                However, the model is loaded with context length of only 1056 tokens, which is not enough. \
                Try to load the model with a larger context length, or provide a shorter input";

            assert!(is_context_length_error(&CLIENT, error).await.unwrap());
        }
        #[tokio::test]
        async fn ignores_rate_limit_error() {
            let error = "API rate limit exceeded. Please try again later.";
            assert!(!is_context_length_error(&CLIENT, error).await.unwrap());
        }

        #[tokio::test]
        async fn ignores_auth_error() {
            let error = "Invalid API key. Please check your credentials and try again.";
            assert!(!is_context_length_error(&CLIENT, error).await.unwrap());
        }

        #[tokio::test]
        async fn ignores_overload_error() {
            let error = "Model 'gpt-4' is currently overloaded. Please try again later.";
            assert!(!is_context_length_error(&CLIENT, error).await.unwrap());
        }
    }
}
