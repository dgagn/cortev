use std::{net::IpAddr, os::fd::FromRawFd};

use axum::{
    extract::{connect_info::Connected, ConnectInfo},
    response::{IntoResponse, Response},
    routing,
    serve::IncomingStream,
    Router,
};
use tokio::{net::TcpListener, signal};

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

    let tcp_listener = if let Ok(listen_fds) = std::env::var("LISTEN_FDS") {
        println!("LISTEN_FDS: {}", listen_fds);
        let listen_fds: i32 = listen_fds.parse().expect("LISTEN_FDS should be 1");
        assert_eq!(listen_fds, 1);
        let raw_fd = 3;
        let std_listener = unsafe { std::net::TcpListener::from_raw_fd(raw_fd) };
        TcpListener::from_std(std_listener).expect("failed to convert to tokio listener")
    } else {
        // local dev
        TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("failed to bind to address")
    };

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
