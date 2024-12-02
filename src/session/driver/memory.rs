use std::{sync::Arc, time::Duration};

use dashmap::DashMap;

use crate::Session;

use super::{SessionData, SessionDriver, SessionError, SessionResult};

#[derive(Debug, Clone)]
pub struct MemoryDriver {
    sessions: Arc<DashMap<String, Session>>,
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
    async fn read(&self, key: &str) -> SessionResult<Session> {
        let session = self.sessions.get(key);
        let session = session.map(|session| {
            session.value().to_owned()
        }).ok_or(SessionError::NotFound)?;
        Ok(session)
    }

    async fn write(&self, key: String, data: SessionData) -> SessionResult<String> {
        let session = Session::builder(key.clone())
            .with_data(data)
            .build();

        self.sessions.insert(key.clone(), session);
        Ok(key)
    }

    async fn destroy(&self, key: &str) -> SessionResult<()> {
        self.sessions.remove(key);
        Ok(())
    }

    fn ttl(&self) -> std::time::Duration {
        self.ttl
    }
}
