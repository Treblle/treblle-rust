use crate::config::AxumConfig;
use crate::extractors::AxumExtractor;
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response},
    middleware::Next,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error};
use treblle_core::{payload::PayloadBuilder, TreblleClient};
use treblle_core::constants::MAX_BODY_SIZE;

#[derive(Clone)]
pub struct TreblleLayer {
    config: Arc<AxumConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleLayer {
    pub fn new(config: AxumConfig) -> Self {
        TreblleLayer {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone()).expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

pub async fn treblle_middleware(
    State(layer): State<Arc<TreblleLayer>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let config = layer.config.clone();
    let treblle_client = layer.treblle_client.clone();
    let start_time = Instant::now();

    let should_process = !config.core.should_ignore_route(req.uri().path())
        && req
        .headers()
        .get("Content-Type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false);

    // Process request for Treblle
    let req = if should_process {
        let (parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(body, MAX_BODY_SIZE)
            .await
            .unwrap_or_default();

        // Store original body for Treblle processing
        let mut new_req = Request::from_parts(parts, Body::from(bytes.clone()));
        new_req.extensions_mut().insert(bytes);
        new_req
    } else {
        req
    };

    if should_process {
        debug!("Processing request for Treblle: {}", req.uri().path());
        let request_payload =
            PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

        let treblle_client_clone = treblle_client.clone();
        tokio::spawn(async move {
            if let Err(e) = treblle_client_clone.send_to_treblle(request_payload).await {
                error!("Failed to send request payload to Treblle: {:?}", e);
            }
        });
    }

    let mut response = next.run(req).await;

    if should_process {
        let duration = start_time.elapsed();

        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, MAX_BODY_SIZE)
            .await
            .unwrap_or_default();

        // Store original body for Treblle processing
        response = Response::from_parts(parts, Body::from(bytes.clone()));
        response.extensions_mut().insert(bytes);

        debug!("Processing response for Treblle: {}", response.status());
        let response_payload = PayloadBuilder::build_response_payload::<AxumExtractor>(
            &response,
            &config.core,
            duration,
        );

        tokio::spawn(async move {
            if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                error!("Failed to send response payload to Treblle: {:?}", e);
            }
        });
    }

    response
}