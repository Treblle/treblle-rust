use std::sync::Arc;
use std::time::Instant;

use rocket::{
    fairing::{Fairing, Info, Kind},
    Data, Request, Response,
};
use serde_json::Value;
use tokio::sync::OnceCell;
use tracing::error;

use crate::config::RocketConfig;
use crate::extractors::TreblleState;
use treblle_core::{
    schema::{LanguageInfo, PayloadData, RequestInfo, ResponseInfo, ServerInfo, TrebllePayload},
    TreblleClient,
};

const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10MB in bytes
static START_TIME: OnceCell<Instant> = OnceCell::const_new();

/// Treblle fairing for Rocket
pub struct TreblleFairing {
    config: Arc<RocketConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleFairing {
    pub fn new(config: RocketConfig) -> Self {
        TreblleFairing {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone())
                    .expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

#[rocket::async_trait]
impl Fairing for TreblleFairing {
    fn info(&self) -> Info {
        Info {
            name: "Treblle",
            kind: Kind::Request | Kind::Response | Kind::Singleton,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, data: &mut Data<'_>) {
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();

        // Only process JSON requests that aren't ignored
        let should_process = !config
            .core
            .ignored_routes
            .iter()
            .any(|route| route.is_match(&req.uri().path().to_string()))
            && req
            .content_type()
            .map(|ct| ct.is_json())
            .unwrap_or(false);

        if should_process {
            if START_TIME.get().is_none() {
                let _ = START_TIME.set(Instant::now());
            }

            // Read request data
            let bytes = data.peek(MAX_JSON_SIZE).await;
            if !bytes.is_empty() {
                if let Ok(json_body) = serde_json::from_slice::<Value>(bytes) {
                    // Store the body in state
                    if let Some(state) = req.rocket().state::<TreblleState>() {
                        if let Ok(mut body) = state.request_body.write() {
                            *body = Some(json_body.clone());
                        }
                    }

                    let payload = TrebllePayload {
                        api_key: config.core.api_key.clone(),
                        project_id: config.core.project_id.clone(),
                        version: 0.1,
                        sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
                        data: PayloadData {
                            server: ServerInfo::default(),
                            language: LanguageInfo {
                                name: "rust".to_string(),
                                version: env!("CARGO_PKG_VERSION").to_string(),
                            },
                            request: RequestInfo {
                                timestamp: chrono::Utc::now(),
                                ip: req.client_ip()
                                    .map(|addr| addr.to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                                url: req.uri().to_string(),
                                method: req.method().to_string(),
                                headers: req.headers().iter()
                                    .map(|h| (h.name.to_string(), h.value.to_string()))
                                    .collect(),
                                user_agent: req.headers().get_one("User-Agent")
                                    .unwrap_or("")
                                    .to_string(),
                                body: Some(json_body),
                            },
                            response: ResponseInfo::default(),
                            errors: Vec::new(),
                        },
                    };

                    let treblle_client_clone = treblle_client.clone();
                    tokio::spawn(async move {
                        if let Err(e) = treblle_client_clone.send_to_treblle(payload).await {
                            error!("Failed to send request payload to Treblle: {:?}", e);
                        }
                    });
                }
            }
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        if let Some(start_time) = START_TIME.get() {
            let duration = start_time.elapsed();
            let treblle_client = self.treblle_client.clone();
            let config = self.config.clone();

            let payload = TrebllePayload {
                api_key: config.core.api_key.clone(),
                project_id: config.core.project_id.clone(),
                version: 0.1,
                sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
                data: PayloadData {
                    server: ServerInfo::default(),
                    language: LanguageInfo {
                        name: "rust".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    request: RequestInfo::default(),
                    response: ResponseInfo {
                        headers: res.headers().iter()
                            .map(|h| (h.name.to_string(), h.value.to_string()))
                            .collect(),
                        code: res.status().code,
                        size: res.headers()
                            .get_one("content-length")
                            .and_then(|val| val.parse::<u64>().ok())
                            .unwrap_or(0),
                        load_time: duration.as_secs_f64(),
                        body: None,
                    },
                    errors: Vec::new(),
                },
            };

            tokio::spawn(async move {
                if let Err(e) = treblle_client.send_to_treblle(payload).await {
                    error!("Failed to send response payload to Treblle: {:?}", e);
                }
            });
        }
    }
}