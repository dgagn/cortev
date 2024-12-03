use axum::{routing, Router};
use cookie::{Cookie, Key};
pub use cortev::session::Session;
use cortev::{
    cookie::{middleware::CookieLayer, CookieJar, CookieKind, CookieMap, EncryptionCookiePolicy},
    session::{
        driver::MemoryDriver,
        middleware::{SessionKind, SessionLayer},
    },
};
use tokio::net::TcpListener;

#[axum::debug_handler]
async fn handler(cookie: CookieJar) -> (CookieJar, &'static str) {
    let cookie = cookie.insert(Cookie::new("theme", "light"));
    (cookie, "Hello, world!")
}

async fn theme(cookie: CookieJar) -> String {
    let cookie = cookie
        .get("theme")
        .unwrap_or_else(|| Cookie::new("theme", "christmas"));
    format!("The theme is {}!", cookie.value())
}

#[tokio::main]
async fn main() {
    let mut encrypted_cookies = CookieMap::new();
    encrypted_cookies.insert("id", CookieKind::Private);
    encrypted_cookies.insert("theme", CookieKind::Private);

    let encryption_policy = EncryptionCookiePolicy::Inclusion(encrypted_cookies);
    let key = Key::generate();
    let jar = CookieJar::builder(key)
        .with_encryption_policy(encryption_policy)
        .build();
    let cookie_layer = CookieLayer::new(jar);
    let driver = MemoryDriver::default();
    let kind = SessionKind::Cookie("id");
    let session_layer = SessionLayer::new(driver, kind);
    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let router = Router::new()
        .route("/", routing::get(handler))
        .route("/theme", routing::get(theme))
        .layer(cookie_layer)
        .layer(session_layer);

    axum::serve(tcp_listener, router).await.unwrap();
}
