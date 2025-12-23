use std::env;

use once_cell::sync::Lazy;
use time::Duration;

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

pub static JWT_ACCESS_LIFETIME: Lazy<Duration> = Lazy::new(|| {
    let s = env::var("JWT_ACCESS_LIFETIME").unwrap_or_else(|_| "5m".into());
    humantime::parse_duration(s.as_str())
        .expect("Invalid JWT_ACCESS_LIFETIME")
        .try_into()
        .expect("JWT_ACCESS_LIFETIME too long")
});

pub static JWT_REFRESH_LIFETIME: Lazy<Option<Duration>> = Lazy::new(|| {
    let s = env::var("JWT_REFRESH_LIFETIME").unwrap_or_else(|_| "1M".into());
    let duration: Duration = humantime::parse_duration(s.as_str())
        .expect("Invalid JWT_ACCESS_LIFETIME")
        .try_into()
        .expect("JWT_ACCESS_LIFETIME too long");
    match duration.is_zero() {
        true => None,
        false => Some(duration),
    }
});

pub fn validate() {
    // Trigger the lazy statics to force panics early
    let _ = &*NAME;
    let _ = &*ROLE_SHORT_DESCRIPTION;
    let _ = &*PERSONALITY_DESCRIPTION;
    let _ = &*JWT_ACCESS_LIFETIME;
    let _ = &*JWT_REFRESH_LIFETIME;
}
