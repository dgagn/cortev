use axum::{routing, Router};
pub use session::Session;
use session::{driver::memory::MemoryDriver, middleware::SessionLayer};
use tokio::net::TcpListener;

pub mod session;
pub mod cookie;

async fn handler(session: Session) -> (Session, &'static str) {
    let session = session.insert("hello", "world");
    (session, "Hello, world!")
}

#[tokio::main]
async fn main() {
    let memory = MemoryDriver::default();
    let session_layer = SessionLayer::new(memory);

    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .layer(session_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
