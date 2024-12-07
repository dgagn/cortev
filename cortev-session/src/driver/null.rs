use crate::{builder::BuildSession, key::SessionKey, Session};

use super::{SessionData, SessionDriver, SessionResult};

#[derive(Debug, Default, Clone)]
pub struct NullDriver {}

impl NullDriver {
    pub fn new() -> Self {
        Self {}
    }
}

impl SessionDriver for NullDriver {
    async fn read(&self, key: SessionKey) -> SessionResult<Option<Session>> {
        let session = Session::builder(key)
            .with_data(SessionData::default())
            .build();
        Ok(Some(session))
    }

    async fn write(&self, key: SessionKey, _data: super::SessionData) -> SessionResult<SessionKey> {
        Ok(key)
    }

    async fn destroy(&self, _key: SessionKey) -> SessionResult<()> {
        Ok(())
    }

    fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }
}
