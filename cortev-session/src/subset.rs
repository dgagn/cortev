use serde_json::Value;

use crate::{state::Transition, Session, SessionData, SessionKey, SessionState};

#[derive(Debug, Clone, Copy)]
pub enum SessionSubsetKind {
    Only,
    Except,
}

/// A subset of session data filtered by specific keys.
///
/// Allows users to work with a subset of a session's data, either including
/// or excluding specified keys based on the subset kind.
#[derive(Debug)]
pub struct SessionSubset<'a, K> {
    /// Reference to the full session data.
    pub(crate) data: &'a SessionData,
    /// The keys used to filter the session data.
    pub(crate) keys: &'a [K],
    /// The kind of subset to create.
    pub(crate) kind: SessionSubsetKind,
    /// Reference to the session's unique key.
    pub(crate) session_key: &'a SessionKey,
    /// The current state of the session associated with this subset.
    pub(crate) state: SessionState,
}

impl<K> SessionSubset<'_, K>
where
    K: AsRef<str>,
{
    /// Checks whether the given `key` exists in the subset based on the filtering rules.
    pub fn has(&self, key: &str) -> bool {
        match self.kind {
            SessionSubsetKind::Only => self.keys.iter().any(|k| k.as_ref() == key),
            SessionSubsetKind::Except => !self.keys.iter().any(|k| k.as_ref() == key),
        }
    }

    /// Retrieves and deserializes the value associated with the given `key` in the subset.
    ///
    /// Returns `Some` if the key is included in the subset and deserialization succeeds.
    pub fn get<V>(&self, key: impl AsRef<str>) -> Option<V>
    where
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.has(key)
            .then(|| self.data.get(key))
            .flatten()
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// Retrieves a reference to the raw value associated with the given `key` in the subset.
    ///
    /// Returns `Some` if the key exists in the subset.
    pub fn get_ref(&self, key: impl AsRef<str>) -> Option<&Value> {
        let key = key.as_ref();
        self.has(key).then(|| self.data.get(key)).flatten()
    }

    /// Retrieves the value associated with the given `key` as a string, if possible.
    ///
    /// Returns `Some` if the key exists and its value is a string.
    pub fn get_str(&self, key: impl AsRef<str>) -> Option<&str> {
        self.get_ref(key).and_then(|value| value.as_str())
    }

    /// Converts this subset into a new session data containing only the filtered data.
    pub fn to_all(&self) -> SessionData {
        self.data
            .iter()
            .filter(|(key, _)| self.has(key.as_ref()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    /// Converts this subset into a new session containing only the filtered data.
    ///
    /// The resulting session inherits the state of the parent session, with
    /// the state transitioned to `Changed`.
    pub fn into_session(self) -> Session {
        Session {
            key: self.session_key.clone(),
            state: self.state.transition(SessionState::Changed),
            data: self.to_all(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subset() {
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

        let session = value.into_session();
        let all = session.all();
        assert_eq!(all.len(), 2);

        let value = all.get("name").unwrap();
        let name = session.get_str("name").unwrap();

        let state = session.state();

        assert_eq!(value, &Value::String("John".into()));
        assert_eq!(name, "John");
        assert_eq!(state, SessionState::Changed);
    }
}
