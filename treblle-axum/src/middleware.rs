use axum::body::Body;
use axum::http::{Request, Response};
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct TreblleLayer {
    api_key: String,
    project_id: String,
}

impl TreblleLayer {
    pub(crate) fn new(api_key: String, project_id: String) -> Self {
        Self { api_key, project_id }
    }
}

impl<S> Layer<S> for TreblleLayer {
    type Service = TreblleMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TreblleMiddleware {
            inner,
            api_key: self.api_key.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

#[derive(Clone)]
pub struct TreblleMiddleware<S> {
    inner: S,
    api_key: String,
    project_id: String,
}

impl<S> Service<Request<Body>> for TreblleMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // TODO: Implement Treblle logic here
        let api_key = self.api_key.clone();
        let project_id = self.project_id.clone();

        let future = self.inner.call(req);
        Box::pin(async move {
            let response = future.await?;
            // TODO: Process response and send data to Treblle
            Ok(response)
        })
    }
}