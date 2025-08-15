#![feature(let_chains, str_as_str, error_generic_member_access)]

use uuid::Uuid;

pub mod config;
pub mod precept;
pub mod precepts;

#[cfg(feature = "infer")]
pub mod infer;

#[cfg(feature = "infer")]
pub mod prompts;
mod orchestra;

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}
