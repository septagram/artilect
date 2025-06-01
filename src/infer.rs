use indoc::formatdoc;
use ouroboros::self_referencing;

pub mod config;
mod error;
pub use error::InferError;
mod openai;
use openai::{ApiError, OpenAIMessage};
use uuid::Uuid;
mod parsing;
use std::sync::Arc;

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
    Continue,
}

impl MessageRole {
    pub fn into_role_str(self) -> Option<&'static str> {
        match self {
            Self::System => Some(openai::ROLE_SYSTEM),
            Self::User => Some(openai::ROLE_USER),
            Self::Assistant => Some(openai::ROLE_ASSISTANT),
            Self::Continue => None,
        }
    }
}

pub struct Message {
    pub role: MessageRole,
    pub content: Box<str>,
}

pub struct ChainItem {
    pub prev: Option<Arc<ChainItem>>,
    pub role: MessageRole,
    pub content: Box<str>,
}

pub struct Chain<'a> {
    id: Uuid,
    client: &'a Client,
    tail: Option<Arc<ChainItem>>,
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

    pub fn push(&mut self, Message { role, content }: Message) {
        if matches!(role, MessageRole::Continue) {
            if self.tail.is_none() {
                // @todo: Rework panic into something better when we figure out
                // multipart messages. Don't want to return Result though.
                panic!("Continue message without any previous messages");
            }
        } else {
            self.message_count += 1;
        }
        self.item_count += 1;
        self.tail = Some(Arc::from(ChainItem {
            prev: self.tail.clone(),
            role,
            content,
        }));
    }

    pub fn with_message(mut self, message: Message) -> Self {
        self.push(message);
        self
    }

    pub fn extend<I>(&mut self, messages: I)
    where
        I: IntoIterator<Item = Message>,
    {
        for message in messages {
            self.push(message);
        }
    }

    pub fn with_messages<I>(mut self, messages: I) -> Self
    where
        I: IntoIterator<Item = Message>,
    {
        self.extend(messages);
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
        {
            let mut new_content = String::from(last_message.content.clone());
            new_content.push_str(&append_str);
            last_message.content = new_content.into();
        }
        match openai::openai_request(&messages, &config::DEFAULT_MODEL, &config::INFER_URL).await {
            Ok(response) => Ok(response),
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
            let reasoning_response = self.clone().with_message(Message {
                    role: MessageRole::Continue,
                    content: crate::system_instructions! {
                        "Do not answer the question just yet, just think out loud, step by step, how it should be answered."
                    },
                }).infer_str(None).await?;
            let value_response = self
                .clone()
                .with_message(Message {
                    role: MessageRole::Assistant,
                    content: format!("\n<think>{reasoning_response}</think>\n\n").into(),
                })
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
        self.push(Message {
            role: MessageRole::Assistant,
            content: value_str,
        });
        Ok(value)
    }

    pub fn as_openai_messages(&self) -> Vec<OpenAIMessage> {
        let mut do_merge_first_user_message = !*config::MODEL_USE_SYSTEM_PROMPT;
        match self.tail.clone() {
            Some(mut cur_item) => {
                let mut messages = Vec::with_capacity(self.message_count);
                let mut cur_content = cur_item.content.as_ref().to_owned();
                let mut cur_role = openai::ROLE_SYSTEM;
                while let Some(prev) = cur_item.prev.clone() {
                    cur_item = prev;
                    let mut role = cur_item.role;
                    if do_merge_first_user_message && matches!(role, MessageRole::User) {
                        do_merge_first_user_message = false;
                        role = MessageRole::Continue;
                    }
                    match role.into_role_str() {
                        Some(role) => {
                            messages.push(OpenAIMessage {
                                role: cur_role,
                                content: std::mem::take(&mut cur_content).into(),
                            });
                            cur_content = cur_item.content.as_ref().to_owned();
                            cur_role = role;
                        }
                        None => {
                            cur_content.push_str(&cur_item.content);
                        }
                    }
                }
                messages.push(OpenAIMessage {
                    role: cur_role,
                    content: cur_content.into(),
                });
                messages.reverse();
                messages
            }
            None => {
                vec![]
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
        .with_message(Message {
            role: MessageRole::User,
            content: formatdoc! {"
                    The following is an error message from OpenAI: {quoted_error}.
                    Is this an error about context length?

                    With no preamble, respond with a JSON object in the following format: {{
                        \"answer\": true if this is a context length error, false otherwise
                    }}
                "}
            .into(),
        })
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
