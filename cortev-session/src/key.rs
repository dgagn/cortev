use std::{fmt, ops::Deref, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey(Arc<str>);

impl SessionKey {
    pub fn new<K: Into<String>>(key: K) -> Self {
        Self(Arc::from(key.into()))
    }
}

impl Deref for SessionKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SessionKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for SessionKey {
    fn from(val: String) -> Self {
        SessionKey::new(val)
    }
}

impl From<&str> for SessionKey {
    fn from(val: &str) -> Self {
        SessionKey::new(val)
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
