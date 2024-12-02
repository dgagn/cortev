use crate::{session::builder::BuildSession, Session};

use super::{SessionData, SessionDriver, SessionResult};

#[derive(Debug, Default, Clone)]
pub struct NullDriver {
}

impl SessionDriver for NullDriver {
    async fn read(&self, key: crate::session::key::SessionKey) -> SessionResult<Session> {
        let session = Session::builder(key)
            .with_data(SessionData::default())
            .build();
        Ok(session)
    }

    async fn write(
        &self,
        key: crate::session::key::SessionKey,
        _data: super::SessionData,
    ) -> SessionResult<crate::session::key::SessionKey> {
        Ok(key)
    }

    async fn destroy(&self, _key: crate::session::key::SessionKey) -> SessionResult<()> {
        Ok(())
    }

    fn ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }
}
