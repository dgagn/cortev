use std::collections::HashMap;

use super::{key::SessionKey, state::SessionState, Session};

#[derive(Debug)]
pub struct WithData;

#[derive(Debug)]
pub struct NoData;

#[derive(Debug)]
pub struct SessionBuilder<State = NoData> {
    key: SessionKey,
    data: Option<HashMap<String, serde_json::Value>>,
    state: std::marker::PhantomData<State>,
}

impl SessionBuilder {
    pub fn new<K: Into<SessionKey>>(key: K) -> Self {
        Self {
            key: key.into(),
            data: None,
            state: Default::default(),
        }
    }
}

impl SessionBuilder<NoData> {
    pub fn with_data(self, data: HashMap<String, serde_json::Value>) -> SessionBuilder<WithData> {
        SessionBuilder {
            key: self.key,
            data: Some(data),
            state: std::marker::PhantomData::<WithData>,
        }
    }
}

impl SessionBuilder<WithData> {
    pub fn build(self) -> Session {
        Session {
            key: self.key,
            // Safe because `WithData` guarantees `data` is set
            data: self.data.unwrap(),
            state: SessionState::Unchanged,
        }
    }
}

pub trait BuildSession {
    fn builder<K: Into<SessionKey>>(key: K) -> SessionBuilder;
}

impl BuildSession for Session {
    fn builder<K: Into<SessionKey>>(key: K) -> SessionBuilder {
        SessionBuilder::new(key)
    }
}
