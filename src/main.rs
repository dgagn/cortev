#![feature(impl_trait_in_assoc_type)]

use axum::{routing, Router};
use session::{driver::memory::MemoryDriver, middleware::SessionLayer};
pub use session::Session;
use tokio::net::TcpListener;

pub mod session;

async fn handler() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() {
    let memory = MemoryDriver::default();
    let session_layer = SessionLayer::new(memory);

    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .layer(session_layer);

    axum::serve(tcp_listener, router)
        .await
        .unwrap();
}
