use dioxus_lib::prelude::*;

mod error;
pub use error::InferError;
mod components;
pub use components::SystemPrompt;
pub mod element_constructors;
mod openai;
use openai::{ApiError, OpenAIMessage};
mod parsing;
pub use parsing::{FromLlmReply, ParseError, PlainText, WithReasoning, YesNoReply};
use std::sync::Arc;
const AGENT_PROMPT_TEXT: &str = "You are the inference agent. \
You are responsible for assisting other agents by solving \
various isolated problems.";

#[macro_export]
macro_rules! prompt {
    ($($tokens:tt)*) => {
        $crate::infer::render_prompt(rsx! { $($tokens)* })
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

pub fn render_prompt(content: Element) -> Result<String, InferError> {
    let xml = dioxus_ssr::render_element(content);
    // TODO: probably it's best to indent the XML as we produce it, but that requires changes to Dioxus SSR.
    // Also we should experiment with how Artilect performs with or w/o indentation.
    if std::env::var("INFER_INDENT_XML").unwrap_or_else(|_| "false".to_string()) == "true" {
        let element = xmltree::Element::parse(xml.as_bytes())
            .map_err(|e| InferError::RenderError(e.to_string()))?;
        let mut output = Vec::new();
        element
            .write_with_config(
                &mut output,
                xmltree::EmitterConfig::new()
                    .perform_indent(true)
                    .write_document_declaration(false),
            )
            .map_err(|e| InferError::RenderError(e.to_string()))?;
        String::from_utf8(output).map_err(|e| InferError::RenderError(e.to_string()))
    } else {
        Ok(xml)
    }
}

pub fn render_system_prompt(agent_system_prompt: &Element) -> Result<String, InferError> {
    prompt! {
        SystemPrompt {
            {agent_system_prompt}
        }
    }
}

pub async fn infer<T: FromLlmReply>(system_prompt: &str, prompt: String) -> Result<T, InferError> {
    let model = std::env::var("DEFAULT_MODEL").unwrap_or_else(|_| "default".to_string());
    let infer_url = std::env::var("INFER_URL").unwrap_or_else(|_| "http://infer".to_string());
    let model_use_system_prompt = std::env::var("MODEL_USE_SYSTEM_PROMPT")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .expect("MODEL_USE_SYSTEM_PROMPT must be 'true' or 'false'");
    let messages = if model_use_system_prompt {
        vec![
            OpenAIMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            OpenAIMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ]
    } else {
        vec![OpenAIMessage {
            role: "user".to_string(),
            content: system_prompt.to_string() + &prompt,
        }]
    };
    let response = match openai::openai_request(messages, model, infer_url).await {
        Ok(response) => Ok(response),
        Err(error) => match error {
            ApiError::ErrorResponse(error_text) => {
                Err(match is_context_length_error(error_text.as_str()).await {
                    Ok(true) => InferError::ContextLengthError(Arc::from(error_text)),
                    Ok(false) => ApiError::ErrorResponse(error_text.clone()).into(),
                    Err(second_error) => {
                        eprintln!(
                            "Warning: Error from is_context_length_error: {:?}",
                            second_error
                        );
                        ApiError::ErrorResponse(error_text.clone()).into()
                    }
                })
            }
            _ => return Err(error.into()),
        },
    };
    Ok(T::from_reply(&response?)?)
}

pub async fn infer_value<T: FromLlmReply>(
    system_prompt: &str,
    prompt: String,
) -> Result<T, InferError> {
    let model_has_reasoning = std::env::var("MODEL_HAS_REASONING")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .expect("MODEL_HAS_REASONING must be 'true' or 'false'");
    if model_has_reasoning {
        infer::<WithReasoning<T>>(system_prompt, prompt)
            .await
            .map(|wr| wr.reply)
    } else {
        infer::<T>(system_prompt, prompt).await
    }
}

#[allow(non_snake_case, non_upper_case_globals)]
pub mod dioxus_elements {
    // pub use dioxus::html::elements::*; // TODO: remove this
    use super::*;

    crate::builder_constructors! {
        instructions None {};
        formatInstructions None {};
    }

    pub mod elements {
        pub use super::*;
    }
}

#[component]
pub fn IsContextLengthPrompt(error: String) -> Element {
    let quoted_error = serde_json::to_string(&error).map_err(|e| RenderError::Aborted(e.into()))?;
    rsx! {
        instructions {
            "The following is an error message from OpenAI: {quoted_error}. ",
            "Is this an error about context length?"
        }
        formatInstructions {
            "With no preamble, respond with a JSON object in the following format: {{\n",
            "    \"answer\": true if this is a context length error, false otherwise\n",
            "}}"
        }
    }
}

pub async fn is_context_length_error(error: &str) -> Result<bool, InferError> {
    let system_prompt = render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}})?;
    Ok(
        Box::pin(infer_value::<YesNoReply>(&system_prompt, prompt! {
            IsContextLengthPrompt {
                error: error.to_string(),
            }
        }?))
        .await?
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_core_macro::component;
    use infer_macros::FromLlmReplyArrayItem;
    use parsing::{FromLlmReplyArray, FromLlmReplyArrayItem};
    use serde::Deserialize;

    mod top_level_array_parsing {
        use super::*;
        #[tokio::test]
        async fn parses_top_level_array_of_strings() {
            let system_prompt = crate::render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}}).unwrap();
            let prompt = prompt! {
                instructions {
                    "Break down the following text into a list of lines, return as JSON array of strings:\n\n",

                    "And you, my father, there on the sad height,\n",
                    "Curse, bless, me now with your fierce tears, I pray.\n",
                    "Do not go gentle into that good night.\n",
                    "Rage, rage against the dying of the light."
                }
                formatInstructions {
                    "With no preamble, respond with a JSON array of strings."
                }
            };
            let result = infer_value::<Vec<Box<str>>>(&system_prompt, prompt.unwrap())
                .await
                .map(|lines| {
                    lines
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

            let system_prompt = crate::render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}}).unwrap();
            let prompt = prompt! {
                instructions {
                    "Here are some interesting celestial objects to consider:\n\n",
                    "The Sun is our home star and the center of our solar system. It has a mass of 1.0 solar masses by definition.\n",
                    "Proxima Centauri b is an exoplanet orbiting the nearest star to our Sun. It's about 1.17 times Earth's mass (0.0000035 solar masses) and lies in the habitable zone.\n",
                    "Betelgeuse is a red supergiant star with approximately 19 solar masses. It's far too hot to be habitable.\n\n",
                    "Parse this information into structured data."
                }
                formatInstructions {
                    "Respond with a JSON array of objects in the following format: {{\n",
                    "    \"name\": the name of the celestial object\n",
                    "    \"mass\": the mass of the celestial object in solar masses\n",
                    "    \"habitable\": true if the celestial object is habitable, false otherwise\n",
                    "}}"
                }
            };

            let result = infer_value::<Vec<SpaceObject>>(&system_prompt, prompt.unwrap()).await;
            assert!(result.is_ok());

            let objects = result.unwrap();
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

            assert!(is_context_length_error(error).await.unwrap());
        }

        #[tokio::test]
        async fn ignores_rate_limit_error() {
            let error = "API rate limit exceeded. Please try again later.";
            assert!(!is_context_length_error(error).await.unwrap());
        }

        #[tokio::test]
        async fn ignores_auth_error() {
            let error = "Invalid API key. Please check your credentials and try again.";
            assert!(!is_context_length_error(error).await.unwrap());
        }

        #[tokio::test]
        async fn ignores_overload_error() {
            let error = "Model 'gpt-4' is currently overloaded. Please try again later.";
            assert!(!is_context_length_error(error).await.unwrap());
        }
    }
}
