use anyhow::Context;
use axum_core::response::{IntoResponse, Response};
use http::StatusCode;
use rand::distributions::{Alphanumeric, DistString};
use std::{collections::HashMap, future::Future, time::Duration};

use super::{key::SessionKey, Session};

pub(crate) type SessionData = HashMap<String, serde_json::Value>;

#[cfg(feature = "redis")]
trait ToJson {
    fn to_json(&self) -> SessionResult<String>;
}

#[cfg(feature = "redis")]
impl ToJson for SessionData {
    fn to_json(&self) -> SessionResult<String> {
        let value = serde_json::to_string(&self).context("failed to serialize session data")?;
        Ok(value)
    }
}

#[cfg(feature = "memory")]
mod memory;
mod null;

// Drivers
#[cfg(feature = "memory")]
pub use memory::MemoryDriver;
pub use null::NullDriver;

type SessionResult<T> = Result<T, SessionError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session was not found")]
    NotFound,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl IntoResponse for SessionError {
    fn into_response(self) -> Response {
        match self {
            SessionError::NotFound => {
                (StatusCode::NOT_FOUND, "session was not found").into_response()
            }
            SessionError::Unexpected(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "unexpected error").into_response()
            }
        }
    }
}

pub trait SessionDriver: Sync {
    fn read(&self, key: SessionKey) -> impl Future<Output = SessionResult<Session>> + Send;
    fn write(
        &self,
        key: SessionKey,
        data: SessionData,
    ) -> impl Future<Output = SessionResult<SessionKey>> + Send;
    fn destroy(&self, key: SessionKey) -> impl Future<Output = SessionResult<()>> + Send;
    fn ttl(&self) -> Duration;

    fn create(&self, data: SessionData) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        let key = generate_random_key();
        self.write(key.into(), data)
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
    ) -> impl Future<Output = SessionResult<SessionKey>> + Send {
        async move {
            self.destroy(key).await?;
            self.init().await
        }
    }
}

/// Generates a random session key.
///
/// [OWASP recommends](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy)
pub fn generate_random_key() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 64)
}
