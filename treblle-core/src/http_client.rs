use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::schema::TrebllePayload;
use crate::error::Result as TreblleResult;
use crate::Config;

pub struct TreblleClient {
    client: Client,
    config: Config,
    current_url_index: AtomicUsize,
}

impl TreblleClient {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
            current_url_index: AtomicUsize::new(0),
        }
    }

    fn get_next_url(&self) -> String {
        let index = self.current_url_index.fetch_add(1, Ordering::SeqCst) % self.config.api_urls.len();
        self.config.api_urls[index].clone()
    }

    pub async fn send_to_treblle(&self, payload: TrebllePayload) -> TreblleResult<()> {
        let url = self.get_next_url();
        let res = self.client
            .post(&url)
            .json(&payload)
            .header("x-api-key", &self.config.api_key)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(crate::error::TreblleError::Http(format!(
                "Failed to send payload to Treblle. Status: {}",
                res.status()
            )));
        }

        Ok(())
    }
}