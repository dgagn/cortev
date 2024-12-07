use axum_core::response::{IntoResponse, Response};
use http::StatusCode;

use crate::driver::SessionError;

pub trait IntoResponseError {
    type Error: std::error::Error + Send + Sync + 'static;
    fn into_response_error(self, error: Self::Error) -> Response;
}

#[derive(Debug, Clone, Copy)]
struct MyError;

impl IntoResponseError for MyError {
    type Error = SessionError;

    fn into_response_error(self, error: SessionError) -> Response {
        tracing::error!("MyError {:?}", error);
        (StatusCode::INTERNAL_SERVER_ERROR, "fudge").into_response()
    }
}
