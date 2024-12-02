pub mod builder;
pub mod driver;
mod key;
pub use key::SessionKey;

pub mod middleware;
mod state;
pub use state::SessionState;

use axum_core::{
    extract::FromRequestParts,
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use http::{request, StatusCode};
use state::Transition;
use std::{collections::HashMap, convert::Infallible};

#[derive(Debug, Clone)]
pub struct Session {
    key: SessionKey,
    state: SessionState,
    data: HashMap<String, serde_json::Value>,
}

impl Session {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn get<K, V>(&self, key: K) -> Option<V>
    where
        K: AsRef<str>,
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.data
            .get(key)
            .and_then(|value| serde_json::from_value(value.to_owned()).ok())
    }

    #[must_use]
    pub fn insert<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        let key = key.into();
        self.data.insert(key, value.into());
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    #[must_use]
    pub fn regenerate(mut self) -> Self {
        self.state = self.state.transition(SessionState::Regenerated);
        self
    }

    #[must_use]
    pub fn invalidate(mut self) -> Self {
        self.state = self.state.transition(SessionState::Invalidated);
        self
    }

    pub fn has<K>(&self, key: K) -> bool
    where
        K: AsRef<str>,
    {
        self.data.contains_key(key.as_ref())
    }

    #[must_use]
    pub fn increment<K>(self, key: K) -> Self
    where
        K: Into<String>,
    {
        self.increment_by(key, 1)
    }

    #[must_use]
    pub fn increment_by<K>(self, key: K, incrementor: i32) -> Self
    where
        K: Into<String>,
    {
        let key = key.into();
        let value: i32 = self.get(&key).unwrap_or(0);
        let value = value + incrementor;
        self.insert(key, value)
    }

    #[must_use]
    pub fn decrement<K>(self, key: K) -> Self
    where
        K: Into<String>,
    {
        self.decrement_by(key, 1)
    }

    #[must_use]
    pub fn decrement_by<K>(self, key: K, decrementor: i32) -> Self
    where
        K: Into<String>,
    {
        self.increment_by(key, -decrementor)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (SessionKey, SessionState, HashMap<String, serde_json::Value>) {
        (self.key, self.state, self.data)
    }
}

impl IntoResponseParts for Session {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let _ = res.extensions_mut().insert(self);
        Ok(res)
    }
}

impl IntoResponse for Session {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Session extension is missing")]
pub struct MissingSessionExtension;

impl IntoResponse for MissingSessionExtension {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync + 'static,
{
    type Rejection = MissingSessionExtension;

    async fn from_request_parts(
        parts: &mut request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(MissingSessionExtension)
    }
}
