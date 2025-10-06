use artilect_macro::{if_precept_in, precept};

pub mod cortex {
    use super::*;
    #[cfg(any(feature = "auth-in", feature = "auth-out", feature = "auth-front"))]
    pub mod auth;
}

pub mod vector {
    use super::*;
    #[precept]
    pub mod chat {
        pub mod dto;
        pub mod front;
        mod local;
        mod remote;

        #[super::if_precept_in(chat)]
        pub use local::{AGENT_PROMPT_TEXT, Precept, ensure_artilect_user};
    }

    #[precept]
    pub mod telegram {
        pub mod dto;
        mod local;

        #[super::if_precept_in(telegram)]
        pub use local::{Precept, Resources};
    }
}
