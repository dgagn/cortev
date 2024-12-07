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
    driver::TokenExt,
    error::{DefaultErrorHandler, IntoErrorResponse, SessionError},
    Session, SessionData, SessionState,
};

use super::driver::SessionDriver;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone)]
pub enum SessionKind {
    Cookie(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct SessionMiddleware<S, D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    inner: S,
    driver: D,
    kind: SessionKind,
    error_handler: H,
}

impl<S, D, H> SessionMiddleware<S, D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    pub fn new(inner: S, driver: D, kind: SessionKind, handler: H) -> Self {
        Self {
            inner,
            driver,
            kind,
            error_handler: handler,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    driver: D,
    kind: SessionKind,
    error_handler: H,
}

impl<D, H> SessionLayer<D, DefaultErrorHandler>
where
    D: SessionDriver,
{
    pub fn builder(driver: D) -> SessionLayerBuilder<D, H> {
        SessionLayerBuilder {
            driver,
            kind: SessionKind::Cookie(Cow::Borrowed("id")),
            error_handler: DefaultErrorHandler,
        }
    }
}

impl<D, H> SessionLayer<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    pub fn new(driver: D, kind: SessionKind, error_handler: H) -> Self {
        Self {
            driver,
            kind,
            error_handler,
        }
    }
}

#[derive(Debug)]
pub struct SessionLayerBuilder<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    driver: D,
    kind: SessionKind,
    error_handler: H,
}

impl<D, H> SessionLayerBuilder<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse<Error = SessionError>,
{
    fn with_kind(self, kind: SessionKind) -> SessionLayerBuilder<D, H> {
        SessionLayerBuilder {
            driver: self.driver,
            kind,
            error_handler: self.error_handler,
        }
    }

    pub fn with_error_handler<HState>(self, handler: HState) -> SessionLayerBuilder<D, HState>
    where
        HState: IntoErrorResponse<Error = SessionError>,
    {
        SessionLayerBuilder {
            driver: self.driver,
            kind: self.kind,
            error_handler: handler,
        }
    }

    pub fn with_cookie<C>(self, name: C) -> SessionLayerBuilder<D, H>
    where
        C: Into<Cow<'static, str>>,
    {
        self.with_kind(SessionKind::Cookie(name.into()))
    }

    pub fn build(self) -> SessionLayer<D, H> {
        SessionLayer::new(self.driver, self.kind, self.error_handler)
    }
}

impl<S, D, H> Layer<S> for SessionLayer<D, H>
where
    D: SessionDriver + Clone,
    H: IntoErrorResponse + Clone,
{
    type Service = SessionMiddleware<S, D, H>;

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

impl<S, D, H> Service<extract::Request> for SessionMiddleware<S, D, H>
where
    S: Service<extract::Request, Response = axum_core::response::Response, Error = Infallible>
        + Clone
        + Send
        + 'static,
    D: SessionDriver + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
    H: IntoErrorResponse<Error = SessionError> + Clone + Send + 'static,
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
        let handler = self.error_handler.clone();
        let future = Box::pin(async move {
            let session_key = match kind {
                SessionKind::Cookie(ref id) => session_cookie(req.headers(), id.clone()),
            };

            let maybe_session = if let Some(cookie) = session_key {
                let key = cookie.value();
                match driver.read(key.into()).await {
                    Ok(session) => session,
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
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
                        #[cfg(feature = "tracing")]
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
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

                #[cfg(feature = "tracing")]
                tracing::debug!("Session state {}", state);

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
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
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
