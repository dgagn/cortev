#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    #[default]
    Unchanged,
    Changed,
    Regenerated,
    Invalidated,
}

#[derive(Debug)]
pub struct Session {
    id: String,
    state: SessionState,
    data: HashMap<String, serde_json::Value>,
}

pub struct WithData;
pub struct WithoutData;

pub struct SessionBuilder<State = WithoutData> {
    id: String,
    data: Option<HashMap<String, serde_json::Value>>,
    state: std::marker::PhantomData<State>,
}

impl SessionBuilder {
    pub fn new(id: String) -> Self {
        Self {
            id,
            data: None,
            state: Default::default(),
        }
    }
}

impl SessionBuilder<WithoutData> {
    pub fn with_data(self, data: HashMap<String, serde_json::Value>) -> SessionBuilder<WithData> {
        SessionBuilder {
            id: self.id,
            data: Some(data),
            state: std::marker::PhantomData::<WithData>,
        }
    }
}

impl SessionBuilder<WithData> {
    pub fn build(self) -> Session {
        Session {
            id: self.id,
            // Safe because `WithData` guarantees `data` is set
            data: self.data.unwrap(),
            state: SessionState::Unchanged,
        }
    }
}

impl Session {
    pub fn builder<K>(id: K) -> SessionBuilder
    where
        K: Into<String>,
    {
        SessionBuilder::new(id.into())
    }

    pub fn id(&self) -> &str {
        &self.id
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
        self.state = SessionState::Changed;
        self
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    #[must_use]
    pub fn regenerate(mut self) -> Self {
        self.state = SessionState::Regenerated;
        self
    }

    #[must_use]
    pub fn invalidate(mut self) -> Self {
        self.state = SessionState::Invalidated;
        self
    }

    pub fn has<K>(&self, key: K) -> bool
    where
        K: AsRef<str>,
    {
        self.data.contains_key(key.as_ref())
    }

    pub(crate) fn into_parts(self) -> (SessionState, HashMap<String, serde_json::Value>) {
        (self.state, self.data)
    }
}
