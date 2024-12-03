use std::{
    convert::Infallible,
    task::{Context, Poll},
};

use axum_core::{
    extract,
    response::{IntoResponse, Response},
};
use futures::FutureExt;
use tower_layer::Layer;
use tower_service::Service;

use crate::CookieJar;

#[derive(Debug, Clone)]
pub struct CookieMidleware<S> {
    inner: S,
    jar: CookieJar,
}

#[derive(Debug, Clone)]
pub struct CookieLayer {
    jar: CookieJar,
}

impl CookieLayer {
    pub fn new(jar: CookieJar) -> Self {
        Self { jar }
    }
}

impl<S> Layer<S> for CookieLayer {
    type Service = CookieMidleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        println!("layer");
        CookieMidleware {
            inner,
            jar: self.jar.clone(),
        }
    }
}

impl<S> CookieMidleware<S> {
    pub fn new(inner: S, jar: CookieJar) -> Self {
        Self { inner, jar }
    }
}

impl<S> Service<extract::Request> for CookieMidleware<S>
where
    S: Service<extract::Request, Response = Response, Error = Infallible> + Clone,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
{
    type Response = Response;
    type Error = Infallible;
    type Future = futures::future::Map<
        S::Future,
        fn(Result<S::Response, Self::Error>) -> Result<S::Response, Self::Error>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        println!("clone");
        let headers = req.headers();
        let jar = self.jar.from_headers(headers);
        req.extensions_mut().insert(jar);

        self.inner.call(req).map(|future| {
            let mut value = match future {
                Ok(response) => response,
                Err(err) => err.into_response(),
            };
            value
                .headers_mut()
                .insert(http::header::AUTHORIZATION, "bob".parse().unwrap());
            Ok(value)
        })
    }
}
