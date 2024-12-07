use std::borrow::Cow;

use tower_layer::Layer;

use crate::{
    driver::{NullDriver, SessionDriver},
    error::{DefaultErrorHandler, IntoErrorResponse},
};

use super::{builder::SessionLayerBuilder, SessionKind, SessionMiddleware};

#[derive(Debug, Clone)]
pub struct SessionLayer<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    driver: D,
    kind: SessionKind,
    error_handler: H,
}

impl SessionLayer<NullDriver, DefaultErrorHandler> {
    pub fn builder() -> SessionLayerBuilder<NullDriver, DefaultErrorHandler> {
        SessionLayerBuilder {
            driver: NullDriver::new(),
            kind: SessionKind::Cookie(Cow::Borrowed("id")),
            error_handler: DefaultErrorHandler,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<D, H> SessionLayer<D, H>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    pub fn new(driver: D, kind: SessionKind, error_handler: H) -> Self {
        Self {
            driver,
            kind,
            error_handler,
        }
    }
}

impl<S, D, H> Layer<S> for SessionLayer<D, H>
where
    D: SessionDriver + Clone,
    H: IntoErrorResponse + Clone,
{
    type Service = SessionMiddleware<S, D, H>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware::new(
            inner,
            self.driver.clone(),
            self.kind.clone(),
            self.error_handler.clone(),
        )
    }
}
