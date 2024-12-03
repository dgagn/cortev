use crate::{kind::CookieKind, CookieKey, CookieMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionCookiePolicy {
    Inclusion(CookieMap),
    Exclusion(CookieMap),
}

impl EncryptionCookiePolicy {
    pub fn inclusion() -> Self {
        Self::Inclusion(CookieMap::new())
    }

    pub fn exclusion() -> Self {
        Self::Exclusion(CookieMap::new())
    }

    pub fn insert<T: Into<CookieKey>>(&mut self, key: T, kind: CookieKind) {
        let key = key.into();
        match self {
            EncryptionCookiePolicy::Inclusion(cookies) => {
                cookies.insert(key, kind);
            }
            EncryptionCookiePolicy::Exclusion(cookies) => {
                cookies.insert(key, kind);
            }
        }
    }

    pub fn cookie_kind<T: Into<CookieKey>>(&self, key: T) -> CookieKind {
        let key = key.into();
        match self {
            EncryptionCookiePolicy::Inclusion(cookies) => {
                cookies.get(&key).unwrap_or(CookieKind::Normal)
            }
            EncryptionCookiePolicy::Exclusion(cookies) => {
                cookies.get(&key).unwrap_or(CookieKind::Private)
            }
        }
    }
}

impl Default for EncryptionCookiePolicy {
    fn default() -> Self {
        Self::Inclusion(CookieMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowlist() {
        let mut cookies = CookieMap::new();
        cookies.insert("session", CookieKind::Private);
        cookies.insert("csrftoken", CookieKind::Signed);
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Inclusion(cookies);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Signed);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
        assert_eq!(policy.cookie_kind("other"), CookieKind::Normal);
    }

    #[test]
    fn test_denylist() {
        let mut cookies = CookieMap::new();
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Exclusion(cookies);
        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
    }

    #[test]
    fn test_default() {
        let policy = EncryptionCookiePolicy::default();

        assert_eq!(policy.cookie_kind("session"), CookieKind::Normal);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Normal);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
    }

    #[test]
    fn test_insert() {
        let mut policy = EncryptionCookiePolicy::default();
        policy.insert("session", CookieKind::Private);
        policy.insert("csrftoken", CookieKind::Signed);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Signed);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
    }

    #[test]
    fn test_insert_exclusion() {
        let mut policy = EncryptionCookiePolicy::exclusion();
        policy.insert("theme", CookieKind::Private);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Private);
    }

    #[test]
    fn test_insert_inclusion() {
        let mut policy = EncryptionCookiePolicy::inclusion();
        policy.insert("session", CookieKind::Private);
        policy.insert("csrftoken", CookieKind::Signed);

        assert_eq!(policy.cookie_kind("session"), CookieKind::Private);
        assert_eq!(policy.cookie_kind("csrftoken"), CookieKind::Signed);
        assert_eq!(policy.cookie_kind("theme"), CookieKind::Normal);
    }
}
