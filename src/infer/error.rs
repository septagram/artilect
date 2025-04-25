use super::openai::{ApiError, OpenAIError};
use super::parsing::ParseError;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InferError {
    #[error("Failed to render prompt: {0}")]
    RenderError(String),

    #[error("LLM API error: {0}")]
    ApiError(#[from] ApiError),

    #[error("Failed to parse LLM response: {0}")]
    ParseError(#[from] ParseError),

    #[error("Context length error: {0}")]
    ContextLengthError(Arc<str>),
}

impl From<dioxus_lib::prelude::RenderError> for InferError {
    fn from(err: dioxus_lib::prelude::RenderError) -> Self {
        InferError::RenderError(err.to_string())
    }
}

impl From<reqwest::Error> for InferError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::from(err).into()
    }
}

impl From<serde_json::Error> for InferError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::from(err).into()
    }
}

impl From<OpenAIError> for InferError {
    fn from(err: OpenAIError) -> Self {
        ApiError::from(err).into()
    }
}
