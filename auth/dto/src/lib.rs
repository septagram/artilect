pub use services::ServiceIdentity;

pub enum Identity {
    User(Uuid),
    Service(ServiceIdentity),
}

pub struct Authenticated<T> (Identity, T);
