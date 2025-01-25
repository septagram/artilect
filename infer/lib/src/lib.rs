use dioxus::prelude::*;

mod error;
pub use error::InferError;
mod components;
pub use components::SystemPrompt;
pub mod element_constructors;
mod openai;
use openai::{ApiError, OpenAIMessage};
mod parsing;
pub use parsing::{FromLlmReply, ParseError, YesNoReply};
use std::sync::Arc;
const AGENT_PROMPT_TEXT: &str = "You are the inference agent. \
You are responsible for assisting other agents by solving \
various isolated problems.";

#[macro_export]
macro_rules! prompt {
    ($($tokens:tt)*) => {
        $crate::render_prompt(rsx! { $($tokens)* })
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
    let messages = vec![
        OpenAIMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        OpenAIMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];
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
    let system_prompt = crate::render_system_prompt(&rsx! {{AGENT_PROMPT_TEXT}})?;
    Ok(
        Box::pin(crate::infer::<YesNoReply>(&system_prompt, prompt! {
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
    use dioxus_core_macro::component;

    use super::*;
}
