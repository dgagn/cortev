use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
    routing, Router,
};
pub use cortev::session::Session;
use cortev::session::{
    driver::MemoryDriver,
    middleware::{SessionKind, SessionLayer},
};
use tokio::net::TcpListener;

async fn handler() -> &'static str {
    "Hello, world!"
}

async fn theme(session: Session) -> String {
    let value: String = session.get("hello").unwrap_or("WTF".into());
    format!("The value in session is {}!", value)
}

async fn login(session: Session) -> (Session, &'static str) {
    let session = session.insert("user_id", 1).regenerate();
    let session = session.regenerate_token();
    (session, "You are logged in!")
}

async fn dashboard(session: Session) -> Response {
    let user_id: i32 = session.get("user_id").unwrap_or(0);
    if user_id != 1 {
        return "You are not logged in!".into_response();
    }
    format!("You are logged in as user {}", user_id).into_response()
}

async fn logout(session: Session) -> (Session, Response) {
    let session = session.invalidate().regenerate_token();
    (session, Redirect::to("/").into_response())
}

#[tokio::main]
async fn main() {
    let driver = MemoryDriver::default();
    let kind = SessionKind::Cookie("id");
    let session_layer = SessionLayer::new(driver, kind);
    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .route("/dashboard", routing::get(dashboard))
        .route("/logout", routing::get(logout))
        .route("/login", routing::get(login))
        .route("/theme", routing::get(theme))
        .layer(session_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
