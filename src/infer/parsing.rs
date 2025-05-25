use serde::Deserialize;
use thiserror::Error;

pub use artilect_macro::FromLlmReply;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),

    #[error("Missing JSON in LLM reply: couldn't find an opening/closing brace pair")]
    MissingJson,

    #[error("Missing reasoning sequence")]
    MissingReasoningSequence,

    #[error("Broken reasoning sequence")]
    BrokenReasoningSequence,
}

pub trait FromLlmReplyArray {
    type Item: Sized + FromLlmReplyArrayItem;
}
pub trait FromLlmReplyArrayItem {}

impl FromLlmReplyArrayItem for Box<str> {}

impl FromLlmReplyArray for Vec<Box<str>> {
    type Item = Box<str>;
}

impl FromLlmReplyArray for std::rc::Rc<[Box<str>]> {
    type Item = Box<str>;
}

impl FromLlmReplyArray for std::sync::Arc<[Box<str>]> {
    type Item = Box<str>;
}

pub trait FromLlmReply {
    fn from_reply(reply: &str) -> Result<Self, ParseError>
    where
        Self: Sized;
}

pub struct PlainText(pub Box<str>);

impl PlainText {
    pub fn get(self) -> Box<str> {
        self.0
    }
}

impl FromLlmReply for PlainText {
    fn from_reply(reply: &str) -> Result<Self, ParseError> {
        Ok(PlainText(reply.into()))
    }
}

// TODO: Refactor WithReasoning for better memory efficiency
//
// - Remove FromLlmReply impl from WithReasoning
// - Store complete LLM response as Box<str>, with reasoning/value_str as &str slices into it
// - Create macro to generate borrowed versions of types using &str instead of Box<str>
// - Add trait for non-consuming parsing from string slices
// - Implement conversion from borrowed to owned versions
//X
// This will enable zero-copy parsing and single allocation for the entire response.

pub struct WithReasoning<T: FromLlmReply> {
    pub value: T,
    pub reasoning: Option<Box<str>>,
}

impl<T: FromLlmReply> WithReasoning<T> {
    pub fn parse(reply: &str) -> Result<(Self, &str, &str), ParseError> {
        const THINK_TAG: &str = "<think>";
        if !reply.starts_with(THINK_TAG) {
            return Err(ParseError::MissingReasoningSequence);
        }
        let reply = &reply[THINK_TAG.len()..];
        match reply.split_once("</think>") {
            Some((reasoning, reply)) => {
                let reasoning = reasoning.trim();
                let reply = reply.trim();
                Ok((
                    WithReasoning {
                        value: T::from_reply(reply.trim())?,
                        reasoning: Some(reasoning.trim().into()),
                    },
                    reasoning,
                    reply,
                ))
            },
            None => Err(ParseError::BrokenReasoningSequence),
        }
    }
}

impl<T: FromLlmReply> FromLlmReply for WithReasoning<T> {
    fn from_reply(reply: &str) -> Result<Self, ParseError> {
        let (value, _, _) = Self::parse(reply)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JsonType {
    Object,
    Array,
}

impl<T> FromLlmReply for T
where
    T: serde::de::DeserializeOwned + FromLlmReplyArray + AsRef<[<T as FromLlmReplyArray>::Item]>,
{
    fn from_reply(reply: &str) -> Result<Self, ParseError> {
        find_and_parse_json(JsonType::Array, reply)
    }
}

pub fn find_and_parse_json<T>(expected_type: JsonType, text: &str) -> Result<T, ParseError> 
where
    T: serde::de::DeserializeOwned,
{
    let (opening_brace, closing_brace) = match expected_type {
        JsonType::Object => ('{', '}'),
        JsonType::Array => ('[', ']'),
    };
    
    let start_index = text.find(opening_brace).ok_or(ParseError::MissingJson)?;
    let end_index = text.rfind(closing_brace).ok_or(ParseError::MissingJson)?;

    let text = &text[start_index..=end_index];
    match serde_json::from_str::<T>(text) {
        Ok(json) => return Ok(json),
        Err(e) => {
            return Err(ParseError::InvalidJson(e));
        }
    }
}

#[derive(FromLlmReply, Deserialize)]
pub struct YesNoReply {
    pub answer: bool,
}

impl From<YesNoReply> for bool {
    fn from(value: YesNoReply) -> Self {
        value.answer
    }
}
