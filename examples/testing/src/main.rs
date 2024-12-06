use std::{ops::Deref, time::Duration};

use axum::{
    debug_handler,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing, Router,
};
pub use cortev::session::Session;
use cortev::session::{driver::RedisDriver, middleware::SessionKind, MissingSessionExtension};
use deadpool_redis::{Config, Runtime};
use tokio::net::TcpListener;

async fn handler() -> &'static str {
    "Hello, world!"
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

#[debug_handler]
async fn logout(session: SessionExt) -> (Session, Response) {
    let session = session.invalidate().regenerate_token();
    (session, Redirect::to("/").into_response())
}

#[derive(Debug, thiserror::Error)]
#[error("my error is happening for extension error")]
pub struct MyError(#[from] MissingSessionExtension);

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

#[derive(Debug, Clone)]
pub struct SessionExt(Session);

#[axum::async_trait]
impl<S> FromRequestParts<S> for SessionExt
where
    S: Send + Sync + 'static,
{
    type Rejection = MyError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Session::from_request_parts(parts, state).await {
            Ok(session) => Ok(Self(session)),
            Err(e) => Err(e.into()),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_url("redis://127.0.0.1:6379");
    let pool = config.create_pool(Some(Runtime::Tokio1)).unwrap();

    let driver = RedisDriver::builder(pool)
        .with_ttl(Duration::from_secs(60 * 60 * 120))
        .with_prefix("session:")
        .build();

    let kind = SessionKind::Cookie("id");
    //let session_layer = SessionLayer::new(driver, kind);

    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .route("/dashboard", routing::get(dashboard))
        .route("/logout", routing::get(logout))
        .route("/login", routing::get(login))
        .route("/theme", routing::get(theme));
    //.layer(session_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
