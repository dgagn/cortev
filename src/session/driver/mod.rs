use std::{collections::HashMap, future::Future, time::Duration};

use anyhow::Context;
use rand::distributions::{Alphanumeric, DistString};

use super::Session;

type SessionData = HashMap<String, serde_json::Value>;

trait Serializable {
    fn serialize(&self) -> SessionResult<String>;
}

impl Serializable for SessionData {
    fn serialize(&self) -> SessionResult<String> {
        let value = serde_json::to_string(&self)
            .context("failed to serialize session data")?;
        Ok(value)
    }
}

#[cfg(feature = "memory")]
pub mod memory;

type SessionResult<T> = Result<T, SessionError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session was not found")]
    NotFound,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

pub trait SessionDriver {
    fn read(&self, key: &str) -> impl Future<Output = SessionResult<Session>> + Send;
    fn write(&self, key: &str, data: SessionData) -> impl Future<Output = SessionResult<String>> + Send;
    fn destroy(&self, key: &str) -> impl Future<Output = SessionResult<()>> + Send;
    fn ttl(&self) -> Duration;
    fn gc(&self) -> impl Future<Output = SessionResult<()>> + Send {
        futures::future::ready(Ok(()))
    }
}

/// Generates a random session key.
///
/// [OWASP recommends](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy)
pub fn generate_random_key() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 64)
}
