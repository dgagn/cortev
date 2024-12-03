use crate::{kind::CookieKind, CookieKey, CookieMap};

#[derive(Debug)]
pub enum EncryptionCookiePolicy {
    Allowlist(CookieMap),
    Denylist(CookieMap),
}

impl EncryptionCookiePolicy {
    pub fn cookie_kind<T: Into<CookieKey>>(&self, key: T) -> CookieKind {
        let key = key.into();
        match self {
            EncryptionCookiePolicy::Allowlist(cookies) => {
                cookies.get(&key).unwrap_or(CookieKind::Normal)
            }
            EncryptionCookiePolicy::Denylist(cookies) => {
                cookies.get(&key).unwrap_or(CookieKind::Private)
            }
        }
    }
}

impl Default for EncryptionCookiePolicy {
    fn default() -> Self {
        Self::Allowlist(CookieMap::new())
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn test_allowlist() {
        let mut cookies = CookieMap::new();
        cookies.insert("session", CookieKind::Private);
        cookies.insert("csrftoken", CookieKind::Signed);
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Allowlist(cookies);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Signed);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
        assert_eq!(policy.cookie_kind("other"), CookieKind::Normal);
    }

    #[test]
    fn test_denylist() {
        let mut cookies = CookieMap::new();
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Denylist(cookies);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
    }
}
