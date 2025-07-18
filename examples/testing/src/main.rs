use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::{
    extract::connect_info::Connected,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing,
    serve::IncomingStream,
    Router,
};
pub use cortev::session::Session;
use cortev::session::{
    driver::RedisDriver,
    error::{IntoErrorResponse, SessionError},
    middleware::SessionLayer,
};
use deadpool_redis::redis::{aio::ConnectionManager, Client};
use tokio::net::TcpListener;

async fn handler() -> Response {
    //let session = session.insert("hello", "world");
    ("Hello, world!").into_response()
}

async fn theme(session: Session) -> String {
    let value: String = session.get("hello").unwrap_or("WTF".into());
    format!("The value in session is {}!", value)
}

async fn login(session: Session) -> (Session, &'static str) {
    let session = session.insert("user_id", 1);
    let session = session.regenerate().regenerate_token();
    (session, "You are logged in!")
}

async fn dashboard(session: Session) -> Response {
    let token = session.token();
    if let Some(token) = token {
        println!("Token: {}", token);
    }
    let user_id: i32 = session.get("user_id").unwrap_or(0);
    if user_id != 1 {
        return "You are not logged in!".into_response();
    }
    format!("You are logged in as user {}", user_id).into_response()
}

async fn logout(session: Session) -> (Session, &'static str) {
    let session = session.invalidate();
    (session, "You are logged out!")
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("fuck an error occured?")]
struct HandleError;

impl IntoErrorResponse for HandleError {
    type Error = SessionError;

    fn into_error_response(self, _error: SessionError) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let client = Client::open("redis+unix:///var/run/redis/redis.sock").unwrap();
    let connection_manager = ConnectionManager::new(client.clone()).await.unwrap();

    let driver = RedisDriver::builder(connection_manager)
        .with_ttl(Duration::from_secs(60 * 60 * 120))
        .with_prefix("session:")
        .build();

    let session = SessionLayer::builder()
        .with_driver(driver)
        .with_cookie("id")
        .with_error_handler(HandleError)
        .build();

    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .route("/dashboard", routing::get(dashboard))
        .route("/logout", routing::get(logout))
        .route("/login", routing::get(login))
        .route("/theme", routing::get(theme))
        .layer(session);

    axum::serve(
        tcp_listener,
        router.into_make_service_with_connect_info::<ClientInfo>(),
    )
    .await
    .unwrap();
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
