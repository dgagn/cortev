use axum::{routing, Router};
use cookie::Key;
pub use cortev::session::Session;
use cortev::{
    cookie::{middleware::CookieLayer, CookieJar, CookieKind, CookieMap, EncryptionCookiePolicy},
    session::{
        driver::NullDriver,
        middleware::{SessionKind, SessionLayer},
    },
};
use tokio::net::TcpListener;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

async fn handler() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() {
    let _profiler = dhat::Profiler::new_heap();
    let mut encrypted_cookies = CookieMap::new();
    encrypted_cookies.insert("id", CookieKind::Private);
    let encryption_policy = EncryptionCookiePolicy::Inclusion(encrypted_cookies);
    let key = Key::generate();
    let jar = CookieJar::builder(key)
        .with_encryption_policy(encryption_policy)
        .build();
    let cookie_layer = CookieLayer::new(jar);
    let driver = NullDriver::default();
    let kind = SessionKind::Cookie("id");
    let session_layer = SessionLayer::new(driver, kind);
    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .layer(cookie_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
