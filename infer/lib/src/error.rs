use crate::openai::OpenAIError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Response parsing failed: {0}")]
    ParseFailed(#[from] serde_json::Error),

    #[error("Error response from API: {0}")]
    ErrorResponse(String),
}

impl From<OpenAIError> for ApiError {
    fn from(err: OpenAIError) -> Self {
        ApiError::ErrorResponse(err.error)
    }
}

#[derive(Error, Debug)]
pub enum InferError {
    #[error("Failed to render prompt: {0}")]
    RenderError(String),

    #[error("LLM API error: {0}")]
    ApiError(#[from] ApiError),

    #[error("Broken reasoning sequence")]
    BrokenReasoningSequence,
}

impl From<dioxus_core::prelude::RenderError> for InferError {
    fn from(err: dioxus_core::prelude::RenderError) -> Self {
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
