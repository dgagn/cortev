use std::future::Future;

use super::Session;

type SessionResult<T> = Result<T, SessionError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session was not found")]
    NotFound,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

pub trait SessionDriver {
    fn read(&self, id: &str) -> impl Future<Output = SessionResult<Session>> + Send;
}

pub struct RedisDriver {
}

impl SessionDriver for RedisDriver {
    async fn read(&self, id: &str) -> SessionResult<Session> {
        todo!()
    }
}
