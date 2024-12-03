use std::sync::Arc;

use cookie::Cookie;
use http::{header, HeaderMap};

mod builder;
mod kind;
mod map;
mod policy;

pub use kind::CookieKind;
pub use map::CookieKey;
pub use map::CookieMap;
pub use policy::EncryptionCookiePolicy;

pub mod middleware;

#[derive(Debug, Clone)]
pub struct CookieJar {
    jar: cookie::CookieJar,
    key: Arc<cookie::Key>,
    encryption_policy: Arc<EncryptionCookiePolicy>,
}

impl CookieJar {
    pub fn builder(key: cookie::Key) -> builder::CookieJarBuilder {
        builder::CookieJarBuilder::new(key)
    }

    pub fn from_headers(&mut self, headers: &HeaderMap) -> Self {
        for cookie in typed_cookies_from_request(headers, &self.encryption_policy) {
            match cookie.kind() {
                CookieKind::Normal => {
                    self.jar.add_original(cookie.into_cookie());
                }
                CookieKind::Private => {
                    self.jar
                        .private_mut(&self.key)
                        .add_original(cookie.into_cookie());
                }
                CookieKind::Signed => {
                    self.jar
                        .signed_mut(&self.key)
                        .add_original(cookie.into_cookie());
                }
            }
        }
        Self {
            // Hashsets are empty so cheap clone
            jar: self.jar.clone(),
            key: self.key.clone(),
            encryption_policy: self.encryption_policy.clone(),
        }
    }
}

#[derive(Debug)]
pub struct TypedCookie<'a> {
    cookie: Cookie<'a>,
    kind: CookieKind,
}

impl<'a> TypedCookie<'a> {
    pub fn into_cookie(self) -> Cookie<'a> {
        self.cookie
    }

    pub fn kind(&self) -> CookieKind {
        self.kind
    }
}

pub fn typed_cookies_from_request<'a>(
    headers: &'a HeaderMap,
    policy: &'a EncryptionCookiePolicy,
) -> impl Iterator<Item = TypedCookie<'static>> + 'a {
    cookies_from_request(headers).map(move |cookie| {
        let kind = policy.cookie_kind(cookie.name().to_owned());
        TypedCookie { cookie, kind }
    })
}

/// Extract cookies from request headers
pub fn cookies_from_request(headers: &HeaderMap) -> impl Iterator<Item = Cookie<'static>> + '_ {
    headers
        .get_all(header::COOKIE)
        .into_iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|cookie| Cookie::parse_encoded(cookie.to_owned()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typed_cookies_from_request() {
        let mut cookies = CookieMap::new();
        cookies.insert("session", CookieKind::Private);
        cookies.insert("csrftoken", CookieKind::Signed);
        cookies.insert("theme", CookieKind::Normal);

        let policy = EncryptionCookiePolicy::Inclusion(cookies);

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "session=1234; csrftoken=5678; theme=light".parse().unwrap(),
        );

        let typed_cookies: Vec<_> = typed_cookies_from_request(&headers, &policy).collect();
        assert_eq!(typed_cookies.len(), 3);

        assert_eq!(typed_cookies[0].kind(), CookieKind::Private);
        assert_eq!(typed_cookies[1].kind(), CookieKind::Signed);
        assert_eq!(typed_cookies[2].kind(), CookieKind::Normal);
    }

    #[test]
    fn test_cookies_from_request() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "session=1234; csrftoken=5678; theme=light".parse().unwrap(),
        );

        let cookies: Vec<_> = cookies_from_request(&headers).collect();
        assert_eq!(cookies.len(), 3);

        assert_eq!(cookies[0].name(), "session");
        assert_eq!(cookies[0].value(), "1234");

        assert_eq!(cookies[1].name(), "csrftoken");
        assert_eq!(cookies[1].value(), "5678");

        assert_eq!(cookies[2].name(), "theme");
        assert_eq!(cookies[2].value(), "light");
    }

    #[test]
    fn test_cookie_jar() {
        let key = cookie::Key::generate();
        let policy = EncryptionCookiePolicy::default();
        let mut jar = CookieJar {
            jar: cookie::CookieJar::new(),
            key: key.into(),
            encryption_policy: policy.into(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "session=1234; csrftoken=5678; theme=light".parse().unwrap(),
        );

        let jar = jar.from_headers(&headers);

        let session = jar.jar.get("session").unwrap();
        assert_eq!(session.value(), "1234");

        let csrftoken = jar.jar.get("csrftoken").unwrap();
        assert_eq!(csrftoken.value(), "5678");

        let theme = jar.jar.get("theme").unwrap();
        assert_eq!(theme.value(), "light");
    }

    #[test]
    fn test_private_cookie_encrypted() {
        let key = cookie::Key::generate();
        let mut policy = EncryptionCookiePolicy::default();
        policy.insert("id", CookieKind::Private);

        let id = create_private_cookie_value(&key, "id", "1234");

        let mut jar = CookieJar {
            jar: cookie::CookieJar::new(),
            key: key.clone().into(),
            encryption_policy: policy.into(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("id={}; csrftoken=5678; theme=light", id)
                .parse()
                .unwrap(),
        );

        let jar = jar.from_headers(&headers);

        let in_private = jar.jar.private(&key).get("id").unwrap();
        let decrypt_cookie = jar.jar.private(&key).decrypt(in_private.clone()).unwrap();
        assert_eq!(decrypt_cookie.value(), "1234");

        let theme = jar.jar.get("theme").unwrap();
        assert_eq!(theme.value(), "light");
    }

    fn create_private_cookie_value(
        key: &cookie::Key,
        id: &'static str,
        value: &'static str,
    ) -> String {
        let mut id_encrypted = cookie::CookieJar::new();
        let mut private_jar = id_encrypted.private_mut(key);
        private_jar.add(Cookie::new(id, value));
        id_encrypted.get(id).unwrap().value().to_owned()
    }

    fn create_signed_cookie_value(
        key: &cookie::Key,
        id: &'static str,
        value: &'static str,
    ) -> String {
        let mut id_encrypted = cookie::CookieJar::new();
        let mut signed_jar = id_encrypted.signed_mut(key);
        signed_jar.add(Cookie::new(id, value));
        id_encrypted.get(id).unwrap().value().to_owned()
    }

    #[test]
    fn test_signed_cookie_encrypted() {
        let key = cookie::Key::generate();
        let mut policy = EncryptionCookiePolicy::default();
        policy.insert("id", CookieKind::Signed);

        let id = create_signed_cookie_value(&key, "id", "1234");

        let mut jar = CookieJar {
            jar: cookie::CookieJar::new(),
            key: key.clone().into(),
            encryption_policy: policy.into(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("id={}; csrftoken=5678; theme=light", id)
                .parse()
                .unwrap(),
        );

        let jar = jar.from_headers(&headers);

        let in_signed = jar.jar.signed(&key).get("id").unwrap();
        let verify_cookie = jar.jar.signed(&key).verify(in_signed.clone()).unwrap();
        assert_eq!(verify_cookie.value(), "1234");

        let theme = jar.jar.get("theme").unwrap();
        assert_eq!(theme.value(), "light");
    }
}
