use axum_core::{
    extract,
    response::{IntoResponse, Response},
    BoxError,
};
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
pub struct SessionMiddleware<S> {
    inner: S,
}

impl<S> SessionMiddleware<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer {}

impl SessionLayer {
    pub fn new() -> Self {
        Self {}
    }
}

impl<S> Layer<S> for SessionLayer {
    type Service = SessionMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(inner)
    }
}

impl<S> Service<extract::Request> for SessionMiddleware<S>
where
    S: Service<extract::Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError> + Send + 'static,
    S::Response: IntoResponse + 'static,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| e.into())
    }

    fn call(&mut self, req: extract::Request) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
        Box::pin(async move {
            // Simulate error here
            let value = 0;

            if value == 0 {
                let error = BoxError::from(SessionError::NotFound);
                return Err(error);
            }

            let response = ready_inner.call(req).await.map_err(|e| e.into())?;
            Ok(response.into_response())
        })
    }
}
