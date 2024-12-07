use axum_core::{extract, response::IntoResponse, response::Response};
use cookie::{time::Duration as CookieDuration, Cookie};
use core::fmt;
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
    driver::{SessionError, TokenExt},
    error::IntoResponseError,
    Session, SessionData, SessionState,
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
pub struct SessionMiddleware<S, D, C, H>
where
    D: SessionDriver,
    C: Into<Cow<'static, str>>,
    H: IntoResponseError,
{
    inner: S,
    driver: D,
    kind: SessionKind<C>,
    error_handler: Option<H>,
}

impl<S, D, C, H> SessionMiddleware<S, D, C, H>
where
    D: SessionDriver,
    C: Into<Cow<'static, str>>,
    H: IntoResponseError,
{
    pub fn new(inner: S, driver: D, kind: SessionKind<C>, handler: Option<H>) -> Self {
        Self {
            inner,
            driver,
            kind,
            error_handler: handler,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer<D, C, H>
where
    D: SessionDriver,
    C: Into<Cow<'static, str>>,
    H: IntoResponseError,
{
    driver: D,
    kind: SessionKind<C>,
    error_handler: Option<H>,
}

impl<D, C, H> SessionLayer<D, C, H>
where
    D: SessionDriver,
    C: Into<Cow<'static, str>>,
    H: IntoResponseError,
{
    pub fn new(driver: D, kind: SessionKind<C>, error_handler: Option<H>) -> Self {
        Self {
            driver,
            kind,
            error_handler,
        }
    }
}

impl<S, D, C, H> Layer<S> for SessionLayer<D, C, H>
where
    D: SessionDriver + Clone,
    C: Into<Cow<'static, str>> + Clone,
    H: IntoResponseError + Clone,
{
    type Service = SessionMiddleware<S, D, C, H>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(
            inner,
            self.driver.clone(),
            self.kind.clone(),
            self.error_handler.clone(),
        )
    }
}

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

impl<S, D, C, H> Service<extract::Request> for SessionMiddleware<S, D, C, H>
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
    H: IntoResponseError<Error = SessionError> + Clone + Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Polling session middleware");

        self.inner.poll_ready(cx)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, fields(http.uri = %req.uri(), http.method = %req.method())))]
    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::debug!("Session middleware called");
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

        let driver = self.driver.clone();
        let kind = self.kind.clone();
        let error_handler = self.error_handler.clone();
        let future = Box::pin(async move {
            let session_key = match kind {
                SessionKind::Cookie(ref id) => session_cookie(req.headers(), id.to_owned()),
            };

            let maybe_session = if let Some(cookie) = session_key {
                let key = cookie.value();
                match driver.read(key.into()).await {
                    Ok(session) => Some(session),
                    Err(SessionError::NotFound) => {
                        #[cfg(feature = "tracing")]
                        tracing::debug!("Session not found");
                        None
                    }
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!("Error reading session: {:?}", err);

                        return if let Some(handler) = error_handler {
                            handler.into_response_error(err)
                        } else {
                            err.into_response()
                        };
                    }
                }
            } else {
                None
            };

            let session = if let Some(session) = maybe_session {
                session
            } else {
                let data = SessionData::session();
                let key = match driver.create(data.clone()).await {
                    Ok(value) => value,
                    Err(err) => {
                        return if let Some(handler) = error_handler {
                            handler.into_response_error(err)
                        } else {
                            err.into_response()
                        };
                    }
                };
                Session::builder(key).with_data(data).build()
            };

            let session_key = session.key.clone();

            req.extensions_mut().insert(session);

            let mut response = match ready_inner.call(req).await {
                Ok(response) => response,
                Err(_err) => unreachable!(), // Infallible
            };

            let extension = response.extensions_mut().remove::<Session>();

            let session_key = if let Some(session) = extension {
                let (key, state, data) = session.into_parts();
                let session_key = match state {
                    SessionState::Changed => driver.write(key, data).await,
                    SessionState::Regenerated => driver.regenerate(key, data).await,
                    SessionState::Invalidated => driver.invalidate(key, data).await,
                    SessionState::Unchanged => Ok(key),
                };
                match session_key {
                    Ok(value) => value,
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!("Session error: {:?}", err);

                        return if let Some(handler) = error_handler {
                            handler.into_response_error(err)
                        } else {
                            err.into_response()
                        };
                    }
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Session not found in response extensions");
                session_key
            };

            let cookie = match kind {
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

            #[cfg(feature = "tracing")]
            tracing::debug!("Session middleware finished");

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
