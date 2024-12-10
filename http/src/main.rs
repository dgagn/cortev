use std::sync::Arc;

use axum::{
    extract::Request,
    response::{IntoResponse, Response},
    routing, Router,
};
use ip::{ClientInfo, TrustedProxies};
use listener::SocketListener;
use middleware::layer::TrustedProxyLayer;
use tokio::signal;

pub mod ip;
pub mod listener;
pub mod middleware;

async fn handler(_request: Request) -> Response {
    let ip = "bob";
    (format!("Hello, {}!", ip)).into_response()
}

#[tokio::main]
async fn main() {
    let trusted_proxies = TrustedProxies::default();
    let layer = TrustedProxyLayer::new(Arc::new(trusted_proxies));

    let router = Router::new().route("/", routing::get(handler)).layer(layer);

    let socket_listener = SocketListener::new("127.0.0.1:8080")
        .await
        .expect("failed to create listener");

    let tcp_listener = socket_listener.into_inner();

    println!("Server started with {}", tcp_listener.local_addr().unwrap());

    let value = router.into_make_service_with_connect_info::<ClientInfo>();

    axum::serve(tcp_listener, value)
        .await
        .expect("failed to start server");

    println!("Server ended");
}
