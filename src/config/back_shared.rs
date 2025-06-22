use once_cell::sync::Lazy;
use std::env;

pub static NAME: Lazy<Box<str>> = Lazy::new(|| {
    env::var("NAME")
        .expect("NAME environment variable must be set")
        .into_boxed_str()
});

pub static ROLE_SHORT_DESCRIPTION: Lazy<Box<str>> = Lazy::new(|| {
    env::var("ROLE_SHORT_DESCRIPTION")
        .unwrap_or_else(|_| "AI companion".into())
        .into_boxed_str()
});

pub static PERSONALITY_DESCRIPTION: Lazy<Box<str>> = Lazy::new(|| {
    env::var("PERSONALITY_DESCRIPTION")
        .unwrap_or_else(|_| "You are helpful, curious, and empathetic.".into())
        .into_boxed_str()
});

pub fn validate() {
    // Trigger the lazy statics to force panics early
    let _ = &*NAME;
    let _ = &*ROLE_SHORT_DESCRIPTION;
    let _ = &*PERSONALITY_DESCRIPTION;
}

