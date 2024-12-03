use axum_core::{extract, response::IntoResponse, response::Response};
use core::fmt;
use std::{
    borrow::Cow,
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

use crate::{builder::BuildSession, driver::SessionData, Session};

use super::driver::SessionDriver;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone)]
pub enum SessionKind<C>
where
    C: Into<Cow<'static, str>>,
{
    Cookie(C),
    EncryptedCookie(C, Vec<u8>),
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
            let key = try_into_response!(driver.init().await);

            #[allow(unused_variables)]
            let value = match kind {
                SessionKind::Cookie(_cow) => "",
                SessionKind::EncryptedCookie(_cow, _) => "",
            };
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
