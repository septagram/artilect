#![feature(str_as_str)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod actuators;
pub mod service;

#[cfg(any(feature = "auth-in", feature = "auth-out", feature = "auth-front"))]
pub mod auth;

#[cfg(feature = "infer")]
pub mod infer;

pub trait Authenticated {
    fn get_from_user_id(&self) -> Uuid;
    fn set_from_user_id(&mut self, id: Uuid);
}

pub trait Identifiable {
    fn get_id(&self) -> Uuid;
}
