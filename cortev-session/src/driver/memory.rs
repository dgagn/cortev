use std::{sync::Arc, time::Duration};

#[cfg(feature = "memory")]
use dashmap::DashMap;

use crate::{builder::BuildSession, key::SessionKey, Session};

use super::{SessionData, SessionDriver, SessionError, SessionResult};

#[derive(Debug, Clone)]
pub struct MemoryDriver {
    sessions: Arc<DashMap<SessionKey, Session>>,
    ttl: Duration,
}

impl Default for MemoryDriver {
    fn default() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            ttl: Duration::from_secs(120 * 60),
        }
    }
}

impl SessionDriver for MemoryDriver {
    async fn read(&self, key: SessionKey) -> SessionResult<Option<Session>> {
        let session = self.sessions.get(&key);
        let session = session.map(|session| session.value().to_owned());
        Ok(session)
    }

    async fn write(&self, key: SessionKey, data: SessionData) -> SessionResult<SessionKey> {
        let session = Session::builder(key.clone()).with_data(data).build();

        self.sessions.insert(key.clone(), session);
        Ok(key)
    }

    async fn destroy(&self, key: SessionKey) -> SessionResult<()> {
        self.sessions.remove(&key);
        Ok(())
    }

    fn ttl(&self) -> std::time::Duration {
        self.ttl
    }
}
