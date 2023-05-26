use std::future::{ready, Ready};

use actix_session::{Session, SessionExt, SessionGetError, SessionInsertError};
use actix_web::{dev::Payload, FromRequest, HttpRequest};

pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "email";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user(&self, email: &str) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, email)
    }

    pub fn get_user(&self) -> Result<Option<String>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }

    pub fn logout(self) {
        self.0.purge()
    }
}

impl FromRequest for TypedSession {
    type Error = <Session as FromRequest>::Error;
    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
