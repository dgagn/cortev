use axum_core::response::{IntoResponse, Response};
use http::StatusCode;

use crate::driver::SessionError;

#[derive(Debug, Clone, Copy)]
struct MyError;

pub trait IntoErrorResponse {
    type Error: std::error::Error + Send + Sync + 'static;
    fn into_error_response(self, error: Self::Error) -> Response;
}

impl IntoErrorResponse for MyError {
    type Error = SessionError;

    fn into_error_response(self, error: SessionError) -> Response {
        tracing::error!("MyError {:?}", error);
        (StatusCode::INTERNAL_SERVER_ERROR, "fudge").into_response()
    }
}
