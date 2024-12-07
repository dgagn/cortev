use axum_core::response::Response;

pub trait IntoErrorResponse {
    type Error: std::error::Error + Send + Sync + 'static;
    fn into_error_response(self, error: Self::Error) -> Response;
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("An error occurred")]
pub struct DefaultErrorResponder;

impl IntoErrorResponse for DefaultErrorResponder {
    type Error = std::convert::Infallible;

    fn into_error_response(self, _error: Self::Error) -> Response {
        todo!()
    }
}
