use cookie::Cookie;
use http::{header, HeaderMap};

mod builder;
mod kind;
mod map;
mod middleware;
mod policy;

pub use kind::CookieKind;
pub use map::CookieKey;
pub use map::CookieMap;
pub use policy::EncryptionCookiePolicy;

#[derive(Debug)]
pub struct CookieJar {
    jar: cookie::CookieJar,
    key: cookie::Key,
    encryption_policy: EncryptionCookiePolicy,
}

impl CookieJar {
    pub fn builder(key: cookie::Key) -> builder::CookieJarBuilder {
        builder::CookieJarBuilder::new(key)
    }

    pub fn from_headers(mut self, headers: &HeaderMap) -> Self {
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
        self
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
    #![allow(unused_imports)]
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
}
