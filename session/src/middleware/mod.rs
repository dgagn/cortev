use std::borrow::Cow;

use crate::error::IntoErrorResponse;

mod builder;
mod cookie;
pub mod future;
mod layer;
mod service;
pub use layer::SessionLayer;

use super::driver::SessionDriver;

#[derive(Debug, Clone)]
pub enum SessionKind {
    Cookie(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct SessionMiddleware<S, D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    inner: S,
    driver: D,
    kind: SessionKind,
    error_handler: H,
}

impl<S, D, H> SessionMiddleware<S, D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    pub fn new(inner: S, driver: D, kind: SessionKind, handler: H) -> Self {
        Self {
            inner,
            driver,
            kind,
            error_handler: handler,
        }
    }
}
