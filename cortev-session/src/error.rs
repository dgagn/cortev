use axum_core::response::{IntoResponse, Response};
use http::StatusCode;

use crate::SessionKey;

pub trait IntoErrorResponse {
    type Error: std::error::Error + Send + Sync + 'static;
    fn into_error_response(self, error: Self::Error) -> Response;
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub enum SessionErrorKind {
    Read,
    Write,
    Destroy,
    Regenerate,
    Invalidate,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[cfg(feature = "redis")]
    #[error("failed to serialize session data")]
    SerializeJson(#[source] serde_json::Error),

    #[cfg(feature = "redis")]
    #[error("failed to serialize session data")]
    DeserializeJson(#[source] serde_json::Error),

    #[cfg(feature = "redis")]
    #[error("cannot acquire a connection from the pool")]
    AcquireConnection(#[source] ::deadpool_redis::PoolError),

    #[cfg(feature = "redis")]
    #[error("redis command error")]
    CommandError(#[from] ::redis::RedisError),

    #[error("cannot {kind} the session data from key {key:?}")]
    SessionKindError {
        #[source]
        source: Box<Self>,
        key: SessionKey,
        kind: SessionErrorKind,
    },

    #[error(transparent)]
    Other(#[from] BoxError),
}

impl IntoResponse for SessionError {
    fn into_response(self) -> Response {
        #[allow(clippy::match_single_binding)]
        match self {
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "500 Internal Server Error",
            )
                .into_response(),
        }
    }
}

#[cfg(feature = "tracing")]
pub(crate) fn log_error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    message.push('\n');

    let mut current = error.source();
    let mut idx = 0;
    if current.is_some() {
        message.push_str("Caused by:");
    }
    while let Some(source) = current {
        message.push_str(&format!("\n{}: {}", idx, source));
        current = source.source();
        idx += 1;
    }

    message
}

#[derive(Debug, thiserror::Error)]
#[error("Session extension is missing")]
pub struct MissingSessionExtension;

impl IntoResponse for MissingSessionExtension {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

impl std::fmt::Display for SessionErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Destroy => write!(f, "destroy"),
            Self::Regenerate => write!(f, "regenerate"),
            Self::Invalidate => write!(f, "invalidate"),
        }
    }
}
