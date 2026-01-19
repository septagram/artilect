use artilect_macro::{if_precept_in, precept};

#[precept(always)]
pub mod auth {
    #[cfg(feature = "server-http2")]
    pub mod middleware;
    #[cfg(feature = "server-http2")]
    pub mod extract;
    pub mod dto;
    pub mod front;
    mod local;
    mod remote;

    pub use dto::User;
    #[cfg(feature = "auth-in")]
    pub use local::AuthFlowBackend;

    #[cfg(feature = "server-http2")]
    pub use extract::*;
}
#[if_precept_in(auth)]
pub use auth::Precept as AuthPrecept;

#[precept]
pub mod chat {
    pub mod dto;
    pub mod front;
    mod local;
    mod remote;

    #[super::if_precept_in(chat)]
    pub use local::{AGENT_PROMPT_TEXT, ensure_artilect_user};
}
#[if_precept_in(chat)]
pub use chat::Precept as ChatPrecept;

#[precept]
pub mod telegram {
    pub mod dto;
    mod local;
}
#[if_precept_in(telegram)]
pub use telegram::Precept as TelegramPrecept;
