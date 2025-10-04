use std::rc::Rc;
use std::sync::Arc;
use artilect_macro::orchestra_from_precepts;
use cfg_block::cfg_block;
use crate::precept::Identity;

orchestra_from_precepts!{
    // auth: cortex::auth,
    chat: vector::chat,
    telegram: vector::telegram,
    // valid ignored comment
}

impl PartialEq for Orchestra {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

cfg_block! {
    #[cfg(feature = "frontend")] {
        use dioxus::prelude::*;

        pub fn use_address_book(orchestra: Arc<Orchestra>, identity: Option<Identity>, token: Option<Arc<str>>) {
            let mut address_book = use_context_provider(|| Signal::new(orchestra.to_address_book(identity.clone(), token.clone())));
            use_effect(use_reactive!(|orchestra, token| {
                let mut write = address_book.write();
                *write = orchestra.to_address_book(identity, token);
            }));
        }
    }
}
