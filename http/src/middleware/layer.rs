use std::{
    convert::Infallible,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    extract::{self, ConnectInfo},
    response::{IntoResponse, Response},
};
use tower_layer::Layer;
use tower_service::Service;

use crate::ip::{ClientInfo, TrustedProxies};

use super::future::ResponseFuture;

#[derive(Debug, Clone)]
pub struct TrustedProxyLayer {
    trusted_proxies: Arc<TrustedProxies>,
}

impl TrustedProxyLayer {
    pub fn new(trusted_proxies: Arc<TrustedProxies>) -> Self {
        Self { trusted_proxies }
    }
}

#[derive(Debug, Clone)]
pub struct TrustedProxyMiddleware<S> {
    inner: S,
    trusted_proxies: Arc<TrustedProxies>,
}

impl<S> Layer<S> for TrustedProxyLayer {
    type Service = TrustedProxyMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrustedProxyMiddleware {
            inner,
            trusted_proxies: self.trusted_proxies.clone(),
        }
    }
}

impl<S> Service<extract::Request> for TrustedProxyMiddleware<S>
where
    S: Service<extract::Request, Response = Response, Error = Infallible>,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        let proxies = Arc::clone(&self.trusted_proxies);
        let ip_addr = req
            .extensions_mut()
            .remove::<ConnectInfo<ClientInfo>>()
            .map(|info| *info);

        if let Some(client_info) = ip_addr {
            if !proxies.is_trusted(client_info.ip()) {
                req.extensions_mut().insert(client_info);
            }
        }

        ResponseFuture {
            future: self.inner.call(req),
        }
    }
}
