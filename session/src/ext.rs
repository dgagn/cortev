use axum_core::extract::Request;
use http::request::Parts;

use crate::Session;

pub trait RequestSessionExt {
    fn session(&self) -> Option<Session>;
    fn take_session(&mut self) -> Option<Session>;
}

impl RequestSessionExt for Request {
    fn session(&self) -> Option<Session> {
        self.extensions().get::<Session>().cloned()
    }

    fn take_session(&mut self) -> Option<Session> {
        self.extensions_mut().remove::<Session>()
    }
}

impl RequestSessionExt for Parts {
    fn session(&self) -> Option<Session> {
        self.extensions.get::<Session>().cloned()
    }

    fn take_session(&mut self) -> Option<Session> {
        self.extensions.remove::<Session>()
    }
}
