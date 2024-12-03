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
        let kind = if policy.is_private(cookie.name().to_owned()) {
            CookieKind::Private
        } else if policy.is_signed(cookie.name().to_owned()) {
            CookieKind::Signed
        } else {
            CookieKind::Normal
        };
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
