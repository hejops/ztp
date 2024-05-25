use std::future::ready;
use std::future::Ready;

use actix_session::Session;
use actix_session::SessionExt;
use actix_session::SessionGetError;
use actix_session::SessionInsertError;
use actix_web::FromRequest;
use uuid::Uuid;

/// Wrapper around `actix_session::Session`, for enabling strict typing (keys
/// are struct fields instead of Strings), and custom methods
pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) { self.0.renew(); }

    pub fn insert_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }
}

impl FromRequest for TypedSession {
    // note the unusual `struct as trait` syntax; 'This is a complicated way of
    // saying "We return the same error returned by the implementation of
    // `FromRequest` for `Session`".'
    //
    // i.e. since `Session` already implements `FromRequest`:
    //      https://docs.rs/actix-session/0.9.0/actix_session/struct.Session.html#impl-FromRequest-for-Session
    //
    // we just reuse its error type
    // https://doc.rust-lang.org/error_codes/E0223.html
    type Error = <Session as FromRequest>::Error;

    type Future = Ready<Result<TypedSession, Self::Error>>;

    // Rust does not yet support the `async` syntax in traits. `from_request`
    // expects a `Future` as return type, to allow for extractors that need to
    // perform asynchronous operations (e.g. a HTTP call).
    //
    // We do not have a `Future`, because session management doesn't require any
    // I/O, so we wrap `TypedSession` into `Ready` to convert it into a `Future`
    // that resolves to the wrapped value the first time it's polled by the
    // executor.

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
