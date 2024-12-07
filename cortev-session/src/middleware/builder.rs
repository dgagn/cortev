use std::borrow::Cow;

use crate::{
    driver::SessionDriver,
    error::{IntoErrorResponse, SessionError},
};

use super::{layer::SessionLayer, SessionKind};

#[derive(Debug)]
pub struct DriverUnset;

#[derive(Debug)]
pub struct DriverSet;

#[derive(Debug)]
pub struct SessionLayerBuilder<D, H, DriverState = DriverUnset>
where
    D: SessionDriver,
    H: IntoErrorResponse,
{
    pub(crate) driver: D,
    pub(crate) kind: SessionKind,
    pub(crate) error_handler: H,
    pub(crate) _marker: std::marker::PhantomData<DriverState>,
}

impl<D, H, DriverState> SessionLayerBuilder<D, H, DriverState>
where
    D: SessionDriver,
    H: IntoErrorResponse<Error = SessionError>,
{
    fn with_kind(self, kind: SessionKind) -> SessionLayerBuilder<D, H, DriverState> {
        SessionLayerBuilder {
            driver: self.driver,
            kind,
            error_handler: self.error_handler,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_error_handler<HState>(
        self,
        handler: HState,
    ) -> SessionLayerBuilder<D, HState, DriverState>
    where
        HState: IntoErrorResponse<Error = SessionError>,
    {
        SessionLayerBuilder {
            driver: self.driver,
            kind: self.kind,
            error_handler: handler,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn with_cookie<C>(self, name: C) -> SessionLayerBuilder<D, H, DriverState>
    where
        C: Into<Cow<'static, str>>,
    {
        self.with_kind(SessionKind::Cookie(name.into()))
    }
}

impl<D, H> SessionLayerBuilder<D, H, DriverSet>
where
    D: SessionDriver,
    H: IntoErrorResponse<Error = SessionError>,
{
    pub fn build(self) -> SessionLayer<D, H> {
        SessionLayer::new(self.driver, self.kind, self.error_handler)
    }
}

impl<D, H> SessionLayerBuilder<D, H, DriverUnset>
where
    D: SessionDriver,
    H: IntoErrorResponse<Error = SessionError>,
{
    pub fn with_driver<DState>(self, driver: DState) -> SessionLayerBuilder<DState, H, DriverSet>
    where
        DState: SessionDriver,
    {
        SessionLayerBuilder {
            driver,
            kind: self.kind,
            error_handler: self.error_handler,
            _marker: std::marker::PhantomData::<DriverSet>,
        }
    }
}
