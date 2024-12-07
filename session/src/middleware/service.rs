use std::convert::Infallible;

use crate::{
    builder::BuildSession,
    driver::{SessionDriver, TokenExt},
    error::{IntoErrorResponse, SessionError},
    middleware::{
        cookie::{session_cookie, set_cookie},
        SessionKind,
    },
    Session, SessionData, SessionState,
};
use axum_core::{
    extract,
    response::{IntoResponse, Response},
};
use cookie::time::Duration as CookieDuration;
use cookie::Cookie;
use tower_service::Service;

use super::{future::ResponseFuture, SessionMiddleware};

impl<S, D, H> Service<extract::Request> for SessionMiddleware<S, D, H>
where
    S: Service<extract::Request, Response = axum_core::response::Response, Error = Infallible>
        + Clone
        + Send
        + 'static,
    D: SessionDriver + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: IntoResponse,
    S::Response: IntoResponse,
    H: IntoErrorResponse<Error = SessionError> + Clone + Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Polling session middleware");

        self.inner.poll_ready(cx)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, fields(http.uri = %req.uri(), http.method = %req.method())))]
    fn call(&mut self, mut req: extract::Request) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::debug!("Session middleware called");
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

        let driver = self.driver.clone();
        let kind = self.kind.clone();
        let handler = self.error_handler.clone();
        let future = Box::pin(async move {
            let session_key = match kind {
                SessionKind::Cookie(ref id) => session_cookie(req.headers(), id.clone()),
            };

            let maybe_session = if let Some(cookie) = session_key {
                let key = cookie.value();
                match driver.read(key.into()).await {
                    Ok(session) => session,
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
                    }
                }
            } else {
                None
            };

            let session = if let Some(session) = maybe_session {
                session
            } else {
                let data = SessionData::session();
                let key = match driver.create(data.clone()).await {
                    Ok(value) => value,
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
                    }
                };
                Session::builder(key).with_data(data).build()
            };

            let session_key = session.key.clone();

            req.extensions_mut().insert(session);

            let mut response = match ready_inner.call(req).await {
                Ok(response) => response,
                Err(_err) => unreachable!(), // Infallible
            };

            let extension = response.extensions_mut().remove::<Session>();

            let session_key = if let Some(session) = extension {
                let (key, state, data) = session.into_parts();

                #[cfg(feature = "tracing")]
                tracing::debug!("Session state {}", state);

                let session_key = match state {
                    SessionState::Changed => driver.write(key, data).await,
                    SessionState::Regenerated => driver.regenerate(key, data).await,
                    SessionState::Invalidated => driver.invalidate(key, data).await,
                    SessionState::Unchanged => Ok(key),
                };
                match session_key {
                    Ok(value) => value,
                    Err(err) => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(error = %crate::error::log_error_chain(&err));

                        return handler.into_error_response(err);
                    }
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Session not found in response extensions");
                session_key
            };

            let cookie = match kind {
                SessionKind::Cookie(id) => {
                    let mut cookie = Cookie::new(id, session_key.to_string());
                    cookie.set_http_only(true);
                    let time = driver.ttl().as_secs();
                    let max_age = CookieDuration::seconds(time as i64);
                    cookie.set_max_age(max_age);
                    cookie
                }
            };

            set_cookie(cookie, response.headers_mut());

            #[cfg(feature = "tracing")]
            tracing::debug!("Session middleware finished");

            response
        });

        ResponseFuture { inner: future }
    }
}
