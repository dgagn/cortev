pub mod inner;

#[derive(Debug, Default)]
pub struct Session {
    inner: inner::Session,
}

impl Session {
    pub fn get<K, V>(&self, key: K) -> Option<V>
    where
        K: AsRef<str>,
        V: serde::de::DeserializeOwned,
    {
        self.inner.get(key)
    }

    #[must_use]
    pub fn insert<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.inner.insert(key, value.into());
        self
    }
}
