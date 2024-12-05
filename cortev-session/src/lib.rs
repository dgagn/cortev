pub mod builder;
pub mod driver;
mod key;
use driver::generate_random_key;
pub use key::SessionKey;

pub mod middleware;
mod state;
use serde_json::Value;
pub use state::SessionState;

use axum_core::{
    extract::FromRequestParts,
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use http::{request, StatusCode};
use state::Transition;
use std::{borrow::Cow, collections::HashMap, convert::Infallible};

pub(crate) type SessionData = HashMap<Cow<'static, str>, Value>;

#[derive(Debug, Clone)]
pub struct Session {
    key: SessionKey,
    state: SessionState,
    data: SessionData,
}

#[derive(Debug, Clone, Copy)]
pub enum SessionSubsetKind {
    Only,
    Except,
}

#[derive(Debug)]
pub struct SessionSubset<'a, K> {
    data: &'a HashMap<Cow<'static, str>, Value>,
    keys: &'a [K],
    kind: SessionSubsetKind,
}

impl<K> SessionSubset<'_, K>
where
    K: AsRef<str>,
{
    fn matches(&self, key: &str) -> bool {
        match self.kind {
            SessionSubsetKind::Only => self.keys.iter().any(|k| k.as_ref() == key),
            SessionSubsetKind::Except => !self.keys.iter().any(|k| k.as_ref() == key),
        }
    }

    pub fn get<V>(&self, key: impl AsRef<str>) -> Option<V>
    where
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.matches(key)
            .then(|| self.data.get(key))
            .flatten()
            .and_then(|value| serde_json::from_value(value.to_owned()).ok())
    }

    pub fn get_ref(&self, key: impl AsRef<str>) -> Option<&Value>
    where
        K: AsRef<str>,
    {
        let key = key.as_ref();
        self.matches(key).then(|| self.data.get(key)).flatten()
    }

    pub fn get_str(&self, key: impl AsRef<str>) -> Option<&str>
    where
        K: AsRef<str>,
    {
        self.get_ref(key).and_then(|value| value.as_str())
    }

    pub fn all(&self) -> HashMap<&str, &Value> {
        self.data
            .iter()
            .filter(|(key, _)| self.matches(key.as_ref()))
            .map(|(key, value)| (key.as_ref(), value))
            .collect()
    }
}

impl Session {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn get<V>(&self, key: impl AsRef<str>) -> Option<V>
    where
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.data
            .get(key)
            .and_then(|value| serde_json::from_value(value.to_owned()).ok())
    }

    pub fn get_ref<K>(&self, key: K) -> Option<&Value>
    where
        K: AsRef<str>,
    {
        self.data.get(key.as_ref())
    }

    pub fn get_str<K>(&self, key: K) -> Option<&str>
    where
        K: AsRef<str>,
    {
        self.get_ref(key).and_then(|value| value.as_str())
    }

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
        self.data.clear();
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
        K: Into<Cow<'static, str>>,
    {
        self.increment_by(key, 1)
    }

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

    #[must_use]
    pub fn decrement<K>(self, key: K) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        self.decrement_by(key, 1)
    }

    #[must_use]
    pub fn decrement_by<K>(self, key: K, decrementor: i32) -> Self
    where
        K: Into<Cow<'static, str>>,
    {
        self.increment_by(key, -decrementor)
    }

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

    pub fn all(&self) -> &SessionData {
        &self.data
    }

    /// Get a subset of the session data.
    pub fn only<'a, K>(&'a self, keys: &'a [K]) -> SessionSubset<'a, K>
    where
        K: AsRef<str>,
    {
        SessionSubset {
            data: &self.data,
            keys,
            kind: SessionSubsetKind::Only,
        }
    }

    /// Get all data except the specified keys.
    pub fn except<'a, K>(&'a self, keys: &'a [K]) -> SessionSubset<'a, K>
    where
        K: AsRef<str>,
    {
        SessionSubset {
            data: &self.data,
            keys,
            kind: SessionSubsetKind::Except,
        }
    }

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

    #[must_use]
    pub fn flush(mut self) -> Self {
        self.data.clear();
        self.state = self.state.transition(SessionState::Changed);
        self
    }

    pub fn token(&self) -> Option<&str> {
        let value = self.data.get("_token");
        let value = value.and_then(|value| value.as_str());
        value
    }

    #[must_use]
    pub fn regenerate_token(mut self) -> Self {
        let token = generate_random_key(40);
        self.data.insert("_token".into(), Value::String(token));
        self.state = self.state.transition(SessionState::Changed);
        self
    }

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
    fn test_subsession_all() {
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

        let all = value.all();
        assert_eq!(all.len(), 2);

        let name = *all.get("name").unwrap();
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
