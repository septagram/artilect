#![feature(let_chains, str_as_str, error_generic_member_access)]

use uuid::Uuid;

pub mod actuators;
pub mod config;
pub mod service;

#[cfg(any(feature = "auth-in", feature = "auth-out", feature = "auth-front"))]
pub mod auth;

#[cfg(feature = "infer")]
pub mod infer;

#[cfg(feature = "infer")]
pub mod prompts;

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}
