use rocket::{
    fairing::{Fairing, Info, Kind},
    Data, Request, Response,
};
use std::sync::Arc;
use std::time::Instant;
use treblle_core::{payload::{PayloadBuilder, HttpExtractor}, TreblleClient};
use crate::extractors::RocketExtractor;
use crate::config::RocketConfig;
use log::{debug, error};

pub struct TreblleHandler {
    config: Arc<RocketConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleHandler {
    pub fn new(config: RocketConfig) -> Self {
        TreblleHandler {
            treblle_client: Arc::new(TreblleClient::new(config.core.clone())),
            config: Arc::new(config),
        }
    }
}

#[rocket::async_trait]
impl Fairing for TreblleHandler {
    fn info(&self) -> Info {
        Info {
            name: "Treblle",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _: &mut Data<'_>) {
        let should_process = !self.config.core.ignored_routes.iter().any(|route| route.is_match(req.uri().path()))
            && req.content_type()
            .map(|ct| ct.is_json())
            .unwrap_or(false);

        if should_process {
            debug!("Processing request for Treblle: {}", req.uri().path());
            let request_payload = PayloadBuilder::build_request_payload::<RocketExtractor>(req, &self.config.core);

            let treblle_client = self.treblle_client.clone();
            tokio::spawn(async move {
                if let Err(e) = treblle_client.send_to_treblle(request_payload).await {
                    error!("Failed to send request payload to Treblle: {:?}", e);
                }
            });
        }

        req.local_cache(|| (Instant::now(), should_process));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let (start_time, should_process) = req.local_cache(|| (Instant::now(), false));

        if *should_process {
            let duration = start_time.elapsed();

            debug!("Processing response for Treblle: {}", res.status());
            let response_payload = PayloadBuilder::build_response_payload::<RocketExtractor>(res, &self.config.core, duration);

            let treblle_client = self.treblle_client.clone();
            tokio::spawn(async move {
                if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                    error!("Failed to send response payload to Treblle: {:?}", e);
                }
            });
        }
    }
}