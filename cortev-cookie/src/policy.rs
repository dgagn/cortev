use crate::{kind::CookieKind, CookieKey, CookieMap};

#[derive(Debug)]
pub enum EncryptionCookiePolicy {
    Allowlist(CookieMap),
    Denylist(CookieMap),
}

impl EncryptionCookiePolicy {
    fn maybe_cookie_kind(&self, key: CookieKey) -> Option<CookieKind> {
        match self {
            EncryptionCookiePolicy::Allowlist(cookies) => cookies.get(&key),
            EncryptionCookiePolicy::Denylist(cookies) => {
                cookies.get(&key).or(Some(CookieKind::Private))
            }
        }
    }

    pub fn is_signed<T: Into<CookieKey>>(&self, key: T) -> bool {
        matches!(self.maybe_cookie_kind(key.into()), Some(CookieKind::Signed))
    }

    pub fn is_private<T: Into<CookieKey>>(&self, key: T) -> bool {
        matches!(
            self.maybe_cookie_kind(key.into()),
            Some(CookieKind::Private)
        )
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

        let policy = EncryptionCookiePolicy::Allowlist(cookies);

        assert!(policy.is_private("session"));
        assert!(!policy.is_private("csrftoken"));
        assert!(!policy.is_private("other"));
        assert!(policy.is_signed("csrftoken"));
    }

    #[test]
    fn test_denylist() {
        let mut cookies = CookieMap::new();
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Denylist(cookies);

        assert!(!policy.is_private("theme"));
        assert!(!policy.is_signed("theme"));

        assert!(policy.is_private("session"));
        assert!(policy.is_private("csrftoken"));
    }
}
