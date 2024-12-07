use rand::distributions::{Alphanumeric, DistString};
use serde_json::Value;
use std::{borrow::Cow, future::Future, time::Duration};

use crate::{error::SessionError, SessionData};

use super::{key::SessionKey, Session};

pub(crate) trait TokenExt {
    fn session() -> Self;
}

impl TokenExt for SessionData {
    fn session() -> Self {
        let mut map = Self::with_capacity(1);
        let token = generate_random_key(40);
        map.insert(Cow::Borrowed("_token"), Value::String(token));
        map
    }
}

#[cfg(feature = "redis")]
pub(crate) trait ToJson {
    fn to_json(&self) -> SessionResult<String>;
}

#[cfg(feature = "redis")]
impl ToJson for SessionData {
    fn to_json(&self) -> SessionResult<String> {
        let value = serde_json::to_string(&self).map_err(SessionError::SerializeJson)?;
        Ok(value)
    }
}

#[cfg(feature = "redis")]
pub(crate) trait FromJson {
    fn from_json(value: &str) -> SessionResult<Self>
    where
        Self: Sized;
}

#[cfg(feature = "redis")]
impl FromJson for SessionData {
    fn from_json(value: &str) -> SessionResult<Self> {
        let value = serde_json::from_str(value).map_err(SessionError::DeserializeJson)?;
        Ok(value)
    }
}

#[cfg(feature = "memory")]
mod memory;
mod null;

#[cfg(feature = "redis")]
mod redis;

// Drivers
#[cfg(feature = "memory")]
pub use memory::MemoryDriver;

#[cfg(feature = "redis")]
pub use redis::{RedisConnectionKind, RedisDriver};

pub use null::NullDriver;

type SessionResult<T> = Result<T, SessionError>;

pub trait SessionDriver: Sync {
    fn read(&self, key: SessionKey) -> impl Future<Output = SessionResult<Option<Session>>> + Send;
    fn write(
        &self,
        key: SessionKey,
        data: SessionData,
    ) -> impl Future<Output = SessionResult<SessionKey>> + Send;
    fn destroy(&self, key: SessionKey) -> impl Future<Output = SessionResult<()>> + Send;
    fn ttl(&self) -> Duration;

    fn create(&self, data: SessionData) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        let key = generate_random_key(64);
        self.write(key.into(), data)
    }

    fn init(&self) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        self.create(SessionData::default())
    }

    fn regenerate(
        &self,
        key: SessionKey,
        data: SessionData,
    ) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        async move {
            let session_key = self.create(data).await?;
            self.destroy(key).await?;
            Ok(session_key)
        }
    }

    fn invalidate(
        &self,
        key: SessionKey,
        data: SessionData,
    ) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        async move {
            self.destroy(key).await?;
            self.create(data).await
        }
    }
}

/// Generates a random session key.
///
/// [OWASP recommends](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy)
pub fn generate_random_key(value: usize) -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), value)
}
