use std::borrow::Cow;

use cookie::Cookie;
use http::{header, HeaderMap};

#[cfg_attr(feature = "tracing", tracing::instrument(skip(headers, cookie_name)))]
pub fn session_cookie(
    headers: &HeaderMap,
    cookie_name: impl Into<Cow<'static, str>>,
) -> Option<Cookie<'_>> {
    let name = cookie_name.into();
    #[cfg(feature = "tracing")]
    tracing::debug!("Looking for session cookie with name {}", name);

    let value = headers
        .get_all(header::COOKIE)
        .into_iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|cookie| {
            let parsed = Cookie::parse_encoded(cookie).ok()?;
            (parsed.name() == name).then_some(parsed)
        });

    value
}

pub(crate) fn set_cookie(cookie: Cookie<'static>, headers: &mut HeaderMap) {
    if let Ok(header_value) = cookie.encoded().to_string().parse() {
        headers.append(header::SET_COOKIE, header_value);
    }
}
