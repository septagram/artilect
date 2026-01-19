#![feature(
    str_as_str,
    error_generic_member_access,
    custom_inner_attributes,
    proc_macro_hygiene,
    const_option_ops,
    try_blocks,
    min_specialization,
    stmt_expr_attributes,
)]

use uuid::Uuid;

pub mod config;
pub mod precept;
pub mod precepts;
pub use precepts::auth;

#[cfg(feature = "infer")]
pub mod infer;

pub mod orchestra;
#[cfg(feature = "infer")]
pub mod prompts;
mod util;

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}
