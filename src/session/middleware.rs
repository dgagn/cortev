use axum_core::{extract, response::IntoResponse, response::Response};
use core::fmt;
use futures::future::BoxFuture;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

use crate::{session::{builder::BuildSession, driver::SessionData}, Session};

use super::driver::SessionDriver;

#[derive(Debug, Clone)]
pub struct SessionMiddleware<S, D: SessionDriver> {
    inner: S,
    driver: D,
}

impl<S, D: SessionDriver> SessionMiddleware<S, D> {
    pub fn new(inner: S, driver: D) -> Self {
        Self { inner, driver }
    }
}

#[derive(Debug, Clone)]
pub struct SessionLayer<D: SessionDriver> {
    driver: D,
}

impl<D: SessionDriver> SessionLayer<D> {
    pub fn new(driver: D) -> Self {
        Self { driver }
    }
}

impl<S, D: SessionDriver + Clone> Layer<S> for SessionLayer<D> {
    type Service = SessionMiddleware<S, D>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(inner, self.driver.clone())
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

impl<S, D> Service<extract::Request> for SessionMiddleware<S, D>
where
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
        let future = Box::pin(async move {
            let key = try_into_response!(driver.init().await);
            println!("session key before response: {}", key);

            let session = Session::builder("helloworld")
                .with_data(SessionData::default())
                .build();

            req.extensions_mut().insert(session);

            let mut response = try_into_response!(ready_inner.call(req).await);

            let extension = response.extensions_mut().remove::<Session>();

            if let Some(session) = extension {
                let (key, state, data) = session.into_parts();
                println!("session key after response: {}", key);
                println!("session state after response: {:?}", state);
                println!("session data after response: {:?}", data);
            } else {
                println!("session is unchanged");
            }

            response
        });

        ResponseFuture { inner: future }
    }
}

pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx).map(Ok)
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}
