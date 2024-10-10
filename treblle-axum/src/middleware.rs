use axum::{
    body::{Body, Bytes},
    http::{Request, Response},
    middleware::Next,
};
use std::sync::Arc;
use std::time::Instant;
use treblle_core::{payload::{PayloadBuilder, HttpExtractor}, TreblleClient};
use crate::extractors::AxumExtractor;
use crate::config::AxumConfig;
use log::{debug, error};

#[derive(Clone)]
pub struct TreblleLayer {
    config: Arc<AxumConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleLayer {
    pub fn new(config: AxumConfig) -> Self {
        TreblleLayer {
            treblle_client: Arc::new(TreblleClient::new(config.core.clone())),
            config: Arc::new(config),
        }
    }
}

pub async fn treblle_middleware<B>(
    req: Request<B>,
    next: Next<B>,
    layer: Arc<TreblleLayer>,
) -> Response<Body>
where
    B: Send + 'static,
{
    let config = layer.config.clone();
    let treblle_client = layer.treblle_client.clone();
    let start_time = Instant::now();

    let should_process = !config.core.ignored_routes.iter().any(|route| route.is_match(req.uri().path()))
        && req.headers().get("Content-Type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false);

    if should_process {
        debug!("Processing request for Treblle: {}", req.uri().path());
        let request_payload = PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

        let treblle_client_clone = treblle_client.clone();
        tokio::spawn(async move {
            if let Err(e) = treblle_client_clone.send_to_treblle(request_payload).await {
                error!("Failed to send request payload to Treblle: {:?}", e);
            }
        });
    }

    let res = next.run(req).await;

    if should_process {
        let duration = start_time.elapsed();
        debug!("Processing response for Treblle: {}", res.status());
        let response_payload = PayloadBuilder::build_response_payload::<AxumExtractor>(&res, &config.core, duration);

        tokio::spawn(async move {
            if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                error!("Failed to send response payload to Treblle: {:?}", e);
            }
        });
    }

    res
}