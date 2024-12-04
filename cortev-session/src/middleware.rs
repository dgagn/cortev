use axum_core::{extract, response::IntoResponse, response::Response};
use core::fmt;
use cortev_cookie::{Cookie, Duration as CookieDuration};
use http::{header, HeaderMap};
use std::{
    borrow::Cow,
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

use crate::{
    builder::BuildSession,
    driver::{SessionData, SessionError},
    Session, SessionState,
};

use super::driver::SessionDriver;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone)]
pub enum SessionKind<C>
where
    C: Into<Cow<'static, str>>,
{
    Cookie(C),
}

#[derive(Debug, Clone)]
pub struct SessionMiddleware<S, D: SessionDriver, C: Into<Cow<'static, str>>> {
    inner: S,
    driver: D,
    kind: SessionKind<C>,
}

impl<S, D: SessionDriver, C: Into<Cow<'static, str>>> SessionMiddleware<S, D, C> {
    pub fn new(inner: S, driver: D, kind: SessionKind<C>) -> Self {
        Self {
            inner,
            driver,
            kind,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer<D: SessionDriver, C: Into<Cow<'static, str>>> {
    driver: D,
    kind: SessionKind<C>,
}

impl<D: SessionDriver, C: Into<Cow<'static, str>>> SessionLayer<D, C> {
    pub fn new(driver: D, kind: SessionKind<C>) -> Self {
        Self { driver, kind }
    }
}

impl<S, D: SessionDriver + Clone, C: Into<Cow<'static, str>> + Clone> Layer<S>
    for SessionLayer<D, C>
{
    type Service = SessionMiddleware<S, D, C>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(inner, self.driver.clone(), self.kind.clone())
    }
}

macro_rules! try_into_response {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(err) => return err.into_response(),
        }
    };
}

pub fn session_cookie(
    headers: &HeaderMap,
    cookie_name: impl Into<Cow<'static, str>>,
) -> Option<Cookie<'_>> {
    let name = cookie_name.into();
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

impl<S, D, C> Service<extract::Request> for SessionMiddleware<S, D, C>
where
    C: Into<Cow<'static, str>> + Clone + Send + 'static,
    S: Service<extract::Request, Response = axum_core::response::Response, Error = Infallible>
        + Clone
        + Send
        + 'static,
    D: SessionDriver + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

        let driver = self.driver.clone();
        let kind = self.kind.clone();
        let future = Box::pin(async move {
            let session_key = match kind {
                SessionKind::Cookie(ref id) => session_cookie(req.headers(), id.clone()),
            };

            let maybe_session = if let Some(cookie) = session_key {
                let key = cookie.value();
                match driver.read(key.into()).await {
                    Ok(session) => Some(session),
                    Err(SessionError::NotFound) => None,
                    Err(err) => return err.into_response(),
                }
            } else {
                None
            };

            let session = if let Some(session) = maybe_session {
                session
            } else {
                let data = SessionData::default();
                let key = try_into_response!(driver.create(data.clone()).await);
                Session::builder(key).with_data(data).build()
            };

            let session_key = session.key.clone();

            req.extensions_mut().insert(session);

            let mut response = try_into_response!(ready_inner.call(req).await);

            let extension = response.extensions_mut().remove::<Session>();

            let session_key = if let Some(session) = extension {
                let (key, state, data) = session.into_parts();
                let session_key = match state {
                    SessionState::Changed => driver.write(key, data).await,
                    SessionState::Regenerated => driver.regenerate(key, data).await,
                    SessionState::Invalidated => driver.invalidate(key, data).await,
                    SessionState::Unchanged => Ok(key),
                };
                try_into_response!(session_key)
            } else {
                session_key
            };

            // todo: Change set the cookie, but for now
            let cookie = match kind {
                #[cfg(feature = "cookie")]
                SessionKind::Cookie(id) => {
                    let mut cookie = Cookie::new(id, session_key.to_string());
                    cookie.set_http_only(true);
                    let time = driver.ttl().as_secs();
                    let max_age = CookieDuration::seconds(time as i64);
                    cookie.set_max_age(max_age);
                    cookie
                }
            };

            set_cookie(cookie, response.headers_mut());

            response
        });

        ResponseFuture { inner: future }
    }
}

fn set_cookie(cookie: Cookie<'static>, headers: &mut HeaderMap) {
    if let Ok(header_value) = cookie.encoded().to_string().parse() {
        headers.append(header::SET_COOKIE, header_value);
    }
}

pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let value = match self.inner.as_mut().poll(cx) {
            Poll::Ready(value) => Poll::Ready(Ok(value)),
            Poll::Pending => Poll::Pending,
        };
        value
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}
