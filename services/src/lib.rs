#[cfg(any(feature = "auth-in", feature = "auth-out"))]
pub mod auth {
    pub mod dto {
        pub use auth_dto::*;
    }
}

#[cfg(any(feature = "chat-in", feature = "chat-out"))]
pub mod chat {
    pub mod dto {
        pub use chat_dto::*;
    }
}

pub enum ServiceIdentity {
    #[actor(auth_back::actor::Auth)]
    Auth,
    #[actor(chat_back::actor::Chat)]
    Chat,
}

pub struct Comms<IdentityType> {
    me: IdentityType,
}

impl<IdentityType> Comms<IdentityType> {
    pub fn new(me: IdentityType) -> Self {
        Self { me }
    }
}

impl<IdentityType> Actor for Comms<IdentityType> {
    // type Message = ServiceMessage;
    type Context = Context<Self>;
}

struct ServiceMessage<IdentityType, MessageType>(IdentityType, MessageType);

// How to infer recipient type?
impl<IdentityType, RecipientType, MessageType, ResultType>
    Handler<ServiceMessage<IdentityType, MessageType>> for Comms<IdentityType>
{
    type Result = ResultType;

    fn handle(&mut self, msg: MessageType, _ctx: &mut Self::Context) -> Self::Result {
        return SystemRegistry::get::<RecipientType>().send(msg);
        todo!()
    }
}
