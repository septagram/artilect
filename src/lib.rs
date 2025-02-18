mod actuators;

#[cfg(any(feature = "auth-in", feature = "auth-out", feature = "auth-front"))]
mod auth;

#[cfg(feature = "infer")]
mod infer;

pub enum ServiceIdentity {
    #[cfg(any(feature = "auth-in", feature = "auth-out"))]
    Auth,
    #[cfg(any(feature = "chat-in", feature = "chat-out"))]
    ChatActuator,
}
