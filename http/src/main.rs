use std::os::fd::FromRawFd;

use axum::{routing, Router};
use tokio::{net::TcpListener, signal, sync::Notify};

async fn handler() -> &'static str {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    "Hello, World!"
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let router = Router::new().route("/", routing::get(handler));

    let notify_shutdown = Arc::new(Notify::new());
    let notify_shutdown_clone = notify_shutdown.clone();

    let tcp_listener = if let Ok(listen_fds) = std::env::var("LISTEN_FDS") {
        println!("LISTEN_FDS: {}", listen_fds);
        let listen_fds: i32 = listen_fds.parse().expect("LISTEN_FDS should be a number");
        if listen_fds == 1 {
            let raw_fd = 3;
            let std_listener = unsafe { std::net::TcpListener::from_raw_fd(raw_fd) };
            TcpListener::from_std(std_listener).expect("failed to convert to tokio listener")
        } else {
            panic!("LISTEN_FDS should be 1")
        }
    } else {
        TcpListener::bind("127.0.0.1:8091")
            .await
            .expect("failed to bind to address")
    };

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("failed to start server");
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

    let sigup = async {
        signal::unix::signal(signal::unix::SignalKind::hangup())
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
        },
        _ = sigup => {
            println!("SIGHUP received");
        }
    }
}
