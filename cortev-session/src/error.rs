use std::convert::Infallible;

use axum_core::{extract, response::Response, BoxError};
use futures::future::BoxFuture;
use tower_layer::Layer;
use tower_service::Service;

#[derive(Debug, Clone)]
pub struct Authorization<T> {
    inner: T,
}

impl<T> Authorization<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner service
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner service
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consume `self`, returning the inner service
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizationLayer {}

impl<S> Layer<S> for AuthorizationLayer {
    type Service = Authorization<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Authorization::new(inner)
    }
}

impl<S> Service<extract::Request> for Authorization<S>
where
    S: Service<extract::Request, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: extract::Request) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await.map_err(Into::into) })
    }
}
