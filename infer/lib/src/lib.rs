use dioxus::prelude::*;
use thiserror::Error;

mod components;
pub mod element_constructors;
mod openai;
pub use components::SystemPrompt;
use openai::OpenAIMessage;

#[macro_export]
macro_rules! prompt {
    ($($tokens:tt)*) => {
        $crate::render_prompt(rsx! { $($tokens)* })
    }
}

#[derive(Error, Debug)]
pub enum InferError {
    #[error("Failed to render prompt: {0}")]
    RenderError(String),

    #[error("LLM API request failed: {0}")]
    ApiError(#[from] reqwest::Error),
}

impl From<dioxus_core::prelude::RenderError> for InferError {
    fn from(err: dioxus_core::prelude::RenderError) -> Self {
        InferError::RenderError(err.to_string())
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

pub async fn infer(system_prompt: &str, prompt: String) -> Result<String, InferError> {
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
    openai::send_openai_request(messages, model, infer_url).await
}

#[cfg(test)]
mod tests {
    use dioxus_core_macro::component;

    use super::*;
}
