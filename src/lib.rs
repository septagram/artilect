#![feature(str_as_str, error_generic_member_access, custom_inner_attributes, proc_macro_hygiene)]

use uuid::Uuid;

pub mod config;
pub mod precept;
pub mod precepts;

#[cfg(feature = "infer")]
pub mod infer;

#[cfg(feature = "infer")]
pub mod prompts;
pub mod orchestra;

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}
