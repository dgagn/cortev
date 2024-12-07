pub mod builder;
pub mod driver;
pub mod ext;
mod key;
use driver::generate_random_key;
use error::SessionMissingFromExt;
use ext::RequestSessionExt;
use http::request::Parts;
pub use key::SessionKey;

pub mod middleware;
mod state;
use serde_json::Value;
pub use state::SessionState;

use axum_core::{
    extract::FromRequestParts,
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use state::Transition;
use std::{borrow::Cow, collections::HashMap, convert::Infallible, ops::Deref};
pub use subset::{SessionSubset, SessionSubsetKind};

pub mod error;
mod subset;

pub(crate) type SessionData = HashMap<Cow<'static, str>, Value>;

/// Represents a user session with data storage and management capabilities.
#[derive(Debug, Clone)]
pub struct Session {
    key: SessionKey,
    state: SessionState,
    data: SessionData,
}

impl Session {
    /// Retrieves the session's key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Gets a value from the session by key and deserializes it into the specified type.
    /// Returns `None` if the key doesn't exist or deserialization fails.
    pub fn get<V>(&self, key: impl AsRef<str>) -> Option<V>
    where
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.data
            .get(key)
            .and_then(|value| serde_json::from_value(value.to_owned()).ok())
    }

    /// Gets a value from the session by key and deserializes it into the specified type.
    /// Returns an error if deserialization fails.
    pub fn try_get<V>(&self, key: impl AsRef<str>) -> Result<V, serde_json::Error>
    where
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        let value = self.data.get(key).cloned().unwrap_or_default();
        serde_json::from_value(value)
    }

    /// Gets a value from the session by key or returns the default value if the key doesn't exist
    /// or deserialization fails.
    pub fn get_or_default<V>(&self, key: impl AsRef<str>) -> V
    where
        V: Default + serde::de::DeserializeOwned,
    {
        self.get(key).unwrap_or_default()
    }

    /// Gets a reference to the raw `Value` associated with the given key.
    pub fn get_ref<K>(&self, key: K) -> Option<&Value>
    where
        K: AsRef<str>,
    {
        self.data.get(key.as_ref())
    }

    /// Gets a string reference for the value associated with the given key.
    pub fn get_str<K>(&self, key: K) -> Option<&str>
    where
        K: AsRef<str>,
    {
        self.get_ref(key).and_then(|value| value.as_str())
    }

    /// Inserts a key-value pair into the session, marking its state as changed.
    #[must_use]
    pub fn insert<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<serde_json::Value>,
    {
        let key = key.into();
        self.data.insert(key, value.into());
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    /// Retrieves the current session state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Marks the session as regenerated and returns the updated session.
    #[must_use]
    pub fn regenerate(mut self) -> Self {
        self.state = self.state.transition(SessionState::Regenerated);
        self
    }

    /// Invalidates the session by clearing its data and marking its state as invalidated.
    #[must_use]
    pub fn invalidate(mut self) -> Self {
        self.data.clear();
        self.state = self.state.transition(SessionState::Invalidated);
        self
    }

    /// Checks if the session contains a specific key.
    pub fn has<K>(&self, key: K) -> bool
    where
        K: AsRef<str>,
    {
        self.data.contains_key(key.as_ref())
    }

    /// Increments the numeric value associated with the key by 1. If the key doesn't exist, it's
    /// initialized to 0 before incrementing.
    #[must_use]
    pub fn increment<K>(self, key: K) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        self.increment_by(key, 1)
    }

    /// Increments the numeric value associated with the key by the specified amount.
    #[must_use]
    pub fn increment_by<K>(self, key: K, incrementor: i32) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        let key = key.into();
        let value: i32 = self.get(&key).unwrap_or(0);
        let value = value + incrementor;
        self.insert(key, value)
    }

    /// Decrements the numeric value associated with the key by 1.
    #[must_use]
    pub fn decrement<K>(self, key: K) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        self.decrement_by(key, 1)
    }

    /// Decrements the numeric value associated with the key by the specified amount.
    #[must_use]
    pub fn decrement_by<K>(self, key: K, decrementor: i32) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        self.increment_by(key, -decrementor)
    }

    /// Removes a key-value pair from the session and marks its state as changed.
    #[must_use]
    pub fn remove<K>(mut self, key: K) -> Self
    where
        K: AsRef<str>,
    {
        let key = key.as_ref();
        self.data.remove(key);
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    /// Retrieves all session data as a reference.
    pub fn all(&self) -> &SessionData {
        &self.data
    }

    /// Retrieves a subset of session data containing only the specified keys.
    pub fn only<'a, K>(&'a self, keys: &'a [K]) -> SessionSubset<'a, K>
    where
        K: AsRef<str>,
    {
        SessionSubset {
            data: &self.data,
            keys,
            kind: SessionSubsetKind::Only,
            state: self.state,
            session_key: &self.key,
        }
    }

    /// Retrieves all session data except the specified keys.
    pub fn except<'a, K>(&'a self, keys: &'a [K]) -> SessionSubset<'a, K>
    where
        K: AsRef<str>,
    {
        SessionSubset {
            data: &self.data,
            keys,
            kind: SessionSubsetKind::Except,
            session_key: &self.key,
            state: self.state,
        }
    }

    /// Removes a key-value pair from the session, returning the updated session and the removed value
    /// (if it existed).
    #[must_use]
    pub fn pull<K>(mut self, key: K) -> (Self, Option<serde_json::Value>)
    where
        K: AsRef<str>,
    {
        let key = key.as_ref();
        let value = self.data.remove(key);
        self.state = self.state.transition(SessionState::Changed);
        (self, value)
    }

    /// Removes multiple keys from the session and marks its state as changed.
    #[must_use]
    pub fn forget<K>(mut self, keys: &[K]) -> Self
    where
        K: AsRef<str>,
    {
        for key in keys {
            let _ = self.data.remove(key.as_ref());
        }
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    /// Clears all session data and marks its state as changed.
    #[must_use]
    pub fn flush(mut self) -> Self {
        self.data.clear();
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    /// Retrieves the session's token value, if present.
    pub fn token(&self) -> Option<&str> {
        let value = self.data.get("_token");
        let value = value.and_then(|value| value.as_str());
        value
    }

    /// Regenerates the session token, marking the session state as changed.
    #[must_use]
    pub fn regenerate_token(mut self) -> Self {
        let token = generate_random_key(40);
        self.data.insert("_token".into(), Value::String(token));
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    /// Decomposes the session into its key, state, and data components.
    pub(crate) fn into_parts(self) -> (SessionKey, SessionState, SessionData) {
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

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync + 'static,
{
    type Rejection = SessionMissingFromExt;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts.take_session().ok_or(SessionMissingFromExt)
    }
}

#[derive(Debug)]
pub struct CloneSession(Session);

impl Deref for CloneSession {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CloneSession {
    pub fn into_inner(self) -> Session {
        self.0
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for CloneSession
where
    S: Send + Sync + 'static,
{
    type Rejection = SessionMissingFromExt;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let session = parts.session().ok_or(SessionMissingFromExt)?;
        Ok(Self(session))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_session() {
        let mut data = SessionData::new();
        data.insert("name".into(), Value::String("John".into()));
        data.insert("age".into(), Value::Number(20.into()));
        data.insert("is_student".into(), Value::Bool(true));
        data.insert("is_teacher".into(), Value::Bool(false));

        let session = Session {
            key: "key".into(),
            state: SessionState::Unchanged,
            data,
        };

        let keys = ["name", "age"];

        let value = session.only(&keys);
        let name = value.get_str("name").unwrap();
        let age = value.get::<i32>("age").unwrap();
        let teacher = value.get::<bool>("is_teacher");

        assert_eq!(name, "John");
        assert_eq!(age, 20);
        assert!(teacher.is_none());
    }

    #[test]
    fn test_except_session() {
        let mut data = SessionData::new();
        data.insert("name".into(), Value::String("John".into()));
        data.insert("age".into(), Value::Number(20.into()));
        data.insert("is_student".into(), Value::Bool(true));
        data.insert("is_teacher".into(), Value::Bool(false));

        let session = Session {
            key: "key".into(),
            state: SessionState::Unchanged,
            data,
        };

        let keys = ["name", "age"];

        let value = session.except(&keys);
        let student = value.get::<bool>("is_student").unwrap();
        let teacher = value.get::<bool>("is_teacher").unwrap();
        let name = value.get_str("name");

        assert!(name.is_none());
        assert!(student);
        assert!(!teacher);
    }

    #[test]
    fn test_session_all() {
        let mut data = SessionData::new();
        data.insert("name".into(), Value::String("John".into()));
        data.insert("age".into(), Value::Number(20.into()));
        data.insert("is_student".into(), Value::Bool(true));
        data.insert("is_teacher".into(), Value::Bool(false));

        let session = Session {
            key: "key".into(),
            state: SessionState::Unchanged,
            data,
        };

        let all = session.all();
        assert_eq!(all.len(), 4);

        let name = all.get("name").unwrap();
        assert_eq!(name, &Value::String("John".into()));
    }

    #[test]
    fn test_session_get() {
        let mut data = SessionData::new();
        data.insert("name".into(), Value::String("John".into()));
        data.insert("age".into(), Value::Number(20.into()));
        data.insert("is_student".into(), Value::Bool(true));
        data.insert("is_teacher".into(), Value::Bool(false));

        let session = Session {
            key: "key".into(),
            state: SessionState::Unchanged,
            data,
        };

        let name = session.get::<String>("name").unwrap();
        let age = session.get::<i32>("age").unwrap();
        let is_student = session.get::<bool>("is_student").unwrap();
        let is_teacher = session.get::<bool>("is_teacher").unwrap();

        assert_eq!(name, "John");
        assert_eq!(age, 20);
        assert!(is_student);
        assert!(!is_teacher);
    }
}
