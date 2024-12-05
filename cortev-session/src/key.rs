use std::{fmt, ops::Deref, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey(Arc<str>);

impl SessionKey {
    pub fn new<K: Into<Arc<str>>>(key: K) -> Self {
        Self(key.into())
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

impl From<SessionKey> for String {
    fn from(val: SessionKey) -> Self {
        val.0.to_string()
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.0.as_ref();
        if key.len() > 24 {
            write!(f, "{}...{}", &key[..8], &key[key.len() - 8..])
        } else {
            write!(f, "{}", key) // If the key is too short, display it fully
        }
    }
}
