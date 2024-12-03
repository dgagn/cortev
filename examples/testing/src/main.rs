use axum::{routing, Router};
pub use cortev::session::Session;
use cortev::session::{
    driver::MemoryDriver,
    middleware::{SessionKind, SessionLayer},
};
use tokio::net::TcpListener;

async fn handler(session: Session) -> (Session, &'static str) {
    let session = session.insert("hello", "world");
    (session, "Hello, world!")
}

#[tokio::main]
async fn main() {
    let driver = MemoryDriver::default();
    let kind = SessionKind::Cookie("id");
    let session_layer = SessionLayer::new(driver, kind);

    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .layer(session_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
