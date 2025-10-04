use artilect_macro::{precept, if_precept_in};

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
        mod local;
        mod remote;
        pub mod front;

        #[super::if_precept_in(chat)]
        pub use local::{Precept, ensure_artilect_user, AGENT_PROMPT_TEXT};
    }

    #[precept]
    pub mod telegram {
        pub mod dto;
        mod local;

        #[super::if_precept_in(telegram)]
        pub use local::{Precept, Resources};
    }
}
