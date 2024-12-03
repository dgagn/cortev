use crate::{kind::CookieKind, CookieKey, CookieKeyMap};

#[derive(Debug)]
pub enum EncryptionCookiePolicy {
    Allowlist(CookieKeyMap),
    Denylist(CookieKeyMap),
}

impl EncryptionCookiePolicy {
    pub fn is_encrypted(&self, key: CookieKey) -> bool {
        match self {
            EncryptionCookiePolicy::Allowlist(cookies) => {
                matches!(cookies.get(&key), Some(CookieKind::Private))
            }
            EncryptionCookiePolicy::Denylist(cookies) => {
                !cookies.has(&key) || matches!(cookies.get(&key), Some(CookieKind::Private))
            }
        }
    }
}

impl Default for EncryptionCookiePolicy {
    fn default() -> Self {
        Self::Allowlist(CookieKeyMap::new())
    }
}
