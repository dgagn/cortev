use std::convert::Infallible;

use axum_core::{extract, response::IntoResponse};
use tower_layer::Layer;
use tower_service::Service;

use super::driver::{SessionStore, SessionManager};

#[derive(Debug, Clone)]
pub struct SessionMiddleware<S, D: SessionManager> {
    inner: S,
    driver: D
}

impl<S, D: SessionManager> SessionMiddleware<S, D> {
    pub fn new(inner: S, driver: D) -> Self {
        Self { inner, driver }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer<D: SessionStore> {
    driver: D
}

impl<D: SessionStore> SessionLayer<D> {
    pub fn new(driver: D) -> Self {
        Self { driver }
    }
}

impl<S, D: SessionManager + Clone> Layer<S> for SessionLayer<D> {
    type Service = SessionMiddleware<S, D>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(inner, self.driver.clone())
    }
}

macro_rules! try_or_response {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(err) => return Ok(err.into_response()),
        }
    };
}

impl<S, D> Service<extract::Request> for SessionMiddleware<S, D>
where
    S: Service<extract::Request, Response = axum_core::response::Response, Error = Infallible> + Clone + Send + 'static,
    D: SessionManager + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
{
    type Response = S::Response;
    type Error = Infallible;
    type Future = impl std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: extract::Request) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

        let driver = self.driver.clone();
        async move {
            let key = try_or_response!(driver.init().await);
            println!("session key before response: {}", key);

            let response = try_or_response!(ready_inner.call(req).await);
            Ok(response)
        }
    }
}
