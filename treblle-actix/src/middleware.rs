use crate::config::ActixConfig;
use crate::extractors::ActixExtractor;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web::Bytes,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    sync::Arc,
    time::Instant,
};
use tracing::{debug, error};
use treblle_core::{PayloadBuilder, TreblleClient};

#[derive(Clone)]
pub struct TreblleMiddleware {
    config: Arc<ActixConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleMiddleware {
    pub fn new(config: ActixConfig) -> Self {
        TreblleMiddleware {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone()).expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

impl<S> Transform<S, ServiceRequest> for TreblleMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Transform = TreblleMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TreblleMiddlewareService {
            service,
            config: self.config.clone(),
            treblle_client: self.treblle_client.clone(),
        }))
    }
}

pub struct TreblleMiddlewareService<S> {
    service: S,
    config: Arc<ActixConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl<S> Service<ServiceRequest> for TreblleMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();
        let start_time = Instant::now();

        let should_process = !config.core.should_ignore_route(req.uri().path())
            && req
                .headers()
                .get("Content-Type")
                .and_then(|ct| ct.to_str().ok())
                .map(|ct| ct.starts_with("application/json"))
                .unwrap_or(false);

        if should_process {
            req.request().extensions_mut().insert(Bytes::new());

            debug!("Processing request for Treblle: {}", req.uri().path());
            let request_payload =
                PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

            let treblle_client_clone = treblle_client.clone();
            actix_web::rt::spawn(async move {
                if let Err(e) = treblle_client_clone.send_to_treblle(request_payload).await {
                    error!("Failed to send request payload to Treblle: {:?}", e);
                }
            });
        }

        let fut = self.service.call(req);
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();

        Box::pin(async move {
            let res = fut.await?;

            if should_process {
                let duration = start_time.elapsed();
                res.request().extensions_mut().insert(Bytes::new());

                debug!("Processing response for Treblle: {}", res.status());
                let response_payload = PayloadBuilder::build_response_payload::<ActixExtractor>(
                    &res,
                    &config.core,
                    duration,
                );

                actix_web::rt::spawn(async move {
                    if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                        error!("Failed to send response payload to Treblle: {:?}", e);
                    }
                });
            }

            Ok(res)
        })
    }
}
