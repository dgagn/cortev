use axum_core::extract::Request;
use http::request::Parts;

use crate::{error::SessionMissingFromExt, Session};

pub trait RequestSessionExt {
    fn try_session(&self) -> Result<Session, SessionMissingFromExt>;
    fn try_take_session(&mut self) -> Result<Session, SessionMissingFromExt>;

    fn session(&self) -> Session {
        self.try_session().unwrap()
    }
    fn take_session(&mut self) -> Session {
        self.try_take_session().unwrap()
    }
}

impl RequestSessionExt for Request {
    fn try_session(&self) -> Result<Session, SessionMissingFromExt> {
        self.extensions()
            .get::<Session>()
            .cloned()
            .ok_or(SessionMissingFromExt)
    }

    fn try_take_session(&mut self) -> Result<Session, SessionMissingFromExt> {
        self.extensions_mut()
            .remove::<Session>()
            .ok_or(SessionMissingFromExt)
    }
}

impl RequestSessionExt for Parts {
    fn try_session(&self) -> Result<Session, SessionMissingFromExt> {
        self.extensions
            .get::<Session>()
            .cloned()
            .ok_or(SessionMissingFromExt)
    }

    fn try_take_session(&mut self) -> Result<Session, SessionMissingFromExt> {
        self.extensions
            .remove::<Session>()
            .ok_or(SessionMissingFromExt)
    }
}
