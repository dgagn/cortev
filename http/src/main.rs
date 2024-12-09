use std::net::IpAddr;

use axum::{
    extract::{connect_info::Connected, ConnectInfo},
    response::{IntoResponse, Response},
    routing,
    serve::IncomingStream,
    Router,
};
use listener::SocketListener;
use tokio::signal;

mod listener;

async fn handler(ConnectInfo(info): ConnectInfo<ClientInfo>) -> Response {
    println!("Connection from: {}", info.ip());
    println!("Handling request");
    println!("Request handled");
    (format!("Hello, {}!", info.ip())).into_response()
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let router = Router::new().route("/", routing::get(handler));

    let socket_listener = SocketListener::new("127.0.0.1:8080")
        .await
        .expect("failed to create listener");

    let tcp_listener = socket_listener.into_inner();

    println!("Server started with {}", tcp_listener.local_addr().unwrap());

    axum::serve(
        tcp_listener,
        router.into_make_service_with_connect_info::<ClientInfo>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("failed to start server");

    println!("Server ended");
}

#[derive(Debug, Clone)]
struct ClientInfo {
    canonical_ip: IpAddr,
}

impl ClientInfo {
    fn ip(&self) -> &IpAddr {
        &self.canonical_ip
    }
}

impl Connected<IncomingStream<'_>> for ClientInfo {
    fn connect_info(stream: IncomingStream<'_>) -> Self {
        ClientInfo {
            canonical_ip: stream.remote_addr().ip().to_canonical(),
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl+C received");
        },
        _ = terminate => {
            println!("SIGTERM received");
        }
    }
}
