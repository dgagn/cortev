use std::{
    convert::Infallible,
    task::{Context, Poll},
};

use axum_core::{
    extract::{self, FromRef, FromRequestParts},
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
};
use cookie::Key;
use futures::FutureExt;
use http::{header, request::Parts, HeaderMap};
use tower_layer::Layer;
use tower_service::Service;

use crate::{CookieJar, EncryptionCookiePolicy};

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
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        println!("CookieMidleware::call");
        let headers = req.headers();
        let jar = self.jar.from_headers(headers);
        req.extensions_mut().insert(jar.clone());

        ResponseFuture {
            future: self.inner.call(req),
            cookie_jar: jar,
        }
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for CookieJar
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // todo: check ways to get from ref from state
        Ok(parts
            .extensions
            .get::<CookieJar>()
            .cloned()
            .expect("the cookie jar is missing"))
    }
}

pin_project_lite::pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<F> {
        #[pin]
        future: F,
        cookie_jar: CookieJar,
    }
}

impl<F> futures::Future for ResponseFuture<F>
where
    F: futures::Future<Output = Result<Response, Infallible>>,
{
    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let res = match this.future.poll(cx) {
            Poll::Ready(output) => output,
            Poll::Pending => return Poll::Pending,
        };

        let mut res = match res {
            Ok(res) => res,
            Err(err) => err.into_response(),
        };

        println!("CookieMidleware::end");

        res.extensions_mut().insert(this.cookie_jar.clone());

        Poll::Ready(Ok(res))
    }
}

impl IntoResponseParts for CookieJar {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        set_cookies(self.jar, res.headers_mut());
        Ok(res)
    }
}

impl IntoResponse for CookieJar {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

fn set_cookies(jar: cookie::CookieJar, headers: &mut HeaderMap) {
    for cookie in jar.delta() {
        if let Ok(header_value) = cookie.encoded().to_string().parse() {
            headers.append(header::SET_COOKIE, header_value);
        }
    }
}
