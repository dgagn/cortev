use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    #[default]
    Unchanged,
    Changed,
    Regenerated,
    Invalidated,
}

#[derive(Debug, Default)]
pub struct Session {
    data: HashMap<String, serde_json::Value>,
    state: SessionState,
}

impl From<HashMap<String, serde_json::Value>> for Session {
    fn from(data: HashMap<String, serde_json::Value>) -> Self {
        Self { data, ..Default::default() }
    }
}

impl Session {
    pub fn get<K, V>(&self, key: K) -> Option<V>
    where
        K: AsRef<str>,
        V: serde::de::DeserializeOwned,
    {
        let key = key.as_ref();
        self.data.get(key).and_then(|value| serde_json::from_value(value.to_owned()).ok())
    }

    pub fn insert<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        let key = key.into();
        self.data.insert(key, value.into());
        self.state = SessionState::Changed;
    }

    pub fn regenerate(&mut self) {
        self.state = SessionState::Regenerated;
    }

    pub fn invalidate(&mut self) {
        self.state = SessionState::Invalidated;
    }
}
