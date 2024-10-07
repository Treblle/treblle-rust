use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Instant,
};

use log::{error, debug};
use reqwest::Client;

use treblle_core::payload::mask_payload;
use treblle_core::schema::TrebllePayload;
use treblle_core::error::Result as TreblleResult;

use crate::config::ActixConfig;
use crate::extractors::ActixExtractor;
use crate::http_client::TreblleClient;

pub struct TreblleMiddleware {
    config: Rc<ActixConfig>,
    treblle_client: TreblleClient,
}

impl TreblleMiddleware {
    pub fn new(config: ActixConfig) -> Self {
        TreblleMiddleware {
            config: Rc::new(config.clone()),
            treblle_client: TreblleClient::new(config.clone()),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TreblleMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TreblleMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TreblleMiddlewareService {
            service,
            config: self.config.clone(),
            treblle_client: self.treblle_client,
        }))
    }
}

pub struct TreblleMiddlewareService<S> {
    service: S,
    config: Rc<ActixConfig>,
    treblle_client: TreblleClient,
}

impl<S, B> Service<ServiceRequest> for TreblleMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let config = self.config.clone();
        let start_time = Instant::now();

        let should_process = !config.core.ignored_routes.iter().any(|route| req.path().starts_with(route));

        if should_process {
            debug!("Processing request for Treblle: {}", req.path());
            match ActixExtractor::build_request_payload(
                config.core.api_key.clone(),
                config.core.project_id.clone(),
                &req,
            ) {
                Ok(mut request_payload) => {
                    mask_payload(&mut request_payload, &config.core.masked_fields);

                    let treblle_client = self.treblle_client.clone();

                    tokio::spawn(async move {
                        if let Err(e) = treblle_client.send_to_treblle(request_payload).await {
                            log::error!("Failed to send request payload to Treblle: {:?}", e);
                        }
                    });
                }
                Err(e) => error!("Failed to build request payload: {e}"),
            }
        }

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            if should_process {
                debug!("Processing response for Treblle: {}", res.status());
                match ActixExtractor::build_response_payload(
                    config.core.api_key.clone(),
                    config.core.project_id.clone(),
                    &res,
                    start_time,
                ) {
                    Ok(mut response_payload) => {
                        mask_payload(&mut response_payload, &config.core.masked_fields);

                        let treblle_client = self.treblle_client.clone();

                        tokio::spawn(async move {
                            if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                                log::error!("Failed to send request payload to Treblle: {:?}", e);
                            }
                        });
                    }
                    Err(e) => error!("Failed to build response payload: {}", e),
                }
            }

            Ok(res)
        })
    }
}