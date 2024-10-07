use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use treblle_core::schema::TrebllePayload;
use treblle_core::error::Result as TreblleResult;
use crate::config::ActixConfig;


pub struct TreblleClient {
    client: Client,
    config: ActixConfig,
    current_url_index: AtomicUsize,
}

impl TreblleClient {
    pub fn new(config: ActixConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            current_url_index: AtomicUsize::new(0),
        }
    }

    fn get_next_url(&self) -> String {
        let index = self.current_url_index.fetch_add(1, Ordering::SeqCst) % self.config.core.api_urls.len();
        self.config.core.api_urls[index].clone()
    }

    async fn send_to_treblle(&self, payload: TrebllePayload) -> TreblleResult<()> {
        let url = self.get_next_url();
        let res = self.client
            .post(&url)
            .json(&payload)
            .header("x-api-key", &self.config.core.api_key)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(treblle_core::error::TreblleError::Http(format!(
                "Failed to send payload to Treblle. Status: {}",
                res.status()
            )));
        }

        Ok(())
    }
}