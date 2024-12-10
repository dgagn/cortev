use std::{
    convert::Infallible,
    task::{Context, Poll},
};

use axum::response::{IntoResponse, Response};

pin_project_lite::pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<F> {
        #[pin]
        pub future: F,
    }
}

impl<F> futures::Future for ResponseFuture<F>
where
    F: futures::Future<Output = Result<Response, Infallible>>,
{
    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let res = match this.future.poll(cx) {
            Poll::Ready(output) => output,
            Poll::Pending => return Poll::Pending,
        };

        let res = match res {
            Ok(res) => res,
            Err(err) => err.into_response(),
        };

        Poll::Ready(Ok(res))
    }
}
