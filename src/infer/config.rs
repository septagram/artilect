use once_cell::sync::Lazy;
use std::env;

pub static DEFAULT_MODEL: Lazy<Box<str>> = Lazy::new(|| {
    env::var("DEFAULT_MODEL")
        .unwrap_or_else(|_| "default".into())
        .into_boxed_str()
});

pub static INFER_URL: Lazy<Box<str>> = Lazy::new(|| {
    env::var("INFER_URL")
        .unwrap_or_else(|_| "http://infer".into())
        .into_boxed_str()
});

pub static MODEL_USE_SYSTEM_PROMPT: Lazy<bool> = Lazy::new(|| {
    env::var("MODEL_USE_SYSTEM_PROMPT")
        .unwrap_or_else(|_| "true".into())
        .parse()
        .expect("MODEL_USE_SYSTEM_PROMPT must be 'true' or 'false'")
});

pub static THINK_ON_POSTFIX: Lazy<Box<str>> = Lazy::new(|| {
    env::var("MODEL_THINK_ON_POSTFIX")
        .unwrap_or_else(|_| "".into())
        .into_boxed_str()
});

pub static THINK_OFF_POSTFIX: Lazy<Box<str>> = Lazy::new(|| {
    env::var("MODEL_THINK_OFF_POSTFIX")
        .unwrap_or_else(|_| "".into())
        .into_boxed_str()
});

pub static MODEL_HAS_REASONING: Lazy<bool> = Lazy::new(|| {
    env::var("MODEL_HAS_REASONING")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .expect("MODEL_HAS_REASONING must be 'true' or 'false'")
});

pub static MODEL_HAS_TOGGLEABLE_REASONING: Lazy<bool> = Lazy::new(|| {
    !THINK_ON_POSTFIX.is_empty() || !THINK_OFF_POSTFIX.is_empty()
});

pub fn validate() {
    // Trigger the lazy statics to force panics early
    let _ = &*DEFAULT_MODEL;
    let _ = &*INFER_URL;
    let _ = *MODEL_USE_SYSTEM_PROMPT;
    let _ = &*THINK_ON_POSTFIX;
    let _ = &*THINK_OFF_POSTFIX;
    let _ = *MODEL_HAS_REASONING;
    let _ = *MODEL_HAS_TOGGLEABLE_REASONING;
}
