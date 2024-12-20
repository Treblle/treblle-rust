use crate::constants::http::REQUEST_TIMEOUT;
use crate::error::{Result as TreblleResult, TreblleError};
use crate::schema::TrebllePayload;
use crate::Config;
use reqwest::{Client, ClientBuilder};
use std::sync::atomic::{AtomicUsize, Ordering};

// First, implement From<reqwest::Error> for TreblleError
impl From<reqwest::Error> for TreblleError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            TreblleError::Timeout
        } else if err.is_connect() {
            TreblleError::Http(format!("Connection error: {err}"))
        } else {
            TreblleError::Http(err.to_string())
        }
    }
}

pub struct TreblleClient {
    client: Client,
    config: Config,
    current_url_index: AtomicUsize,
}

impl TreblleClient {
    pub fn new(config: Config) -> TreblleResult<Self> {
        let client = ClientBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| TreblleError::Http(format!("Failed to create HTTP client: {e}")))?;
        Ok(Self { client, config, current_url_index: AtomicUsize::new(0) })
    }

    fn get_next_url(&self) -> String {
        let index =
            self.current_url_index.fetch_add(1, Ordering::SeqCst) % self.config.api_urls.len();
        self.config.api_urls[index].clone()
    }

    pub async fn send_to_treblle(&self, payload: TrebllePayload) -> TreblleResult<()> {
        let url = self.get_next_url();

        // Fire and forget approach - we don't wait for the response
        let _ = self
            .client
            .post(&url)
            .json(&payload)
            .header("x-api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PayloadData;
    use std::time::Duration;
    use tokio;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_client_rotation() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let client = TreblleClient::new(config).unwrap();

        // Test URL rotation
        let first_url = client.get_next_url();
        let second_url = client.get_next_url();
        let _third_url = client.get_next_url(); // Just to trigger rotation
        let fourth_url = client.get_next_url(); // Should wrap around to 1st URL

        assert_ne!(first_url, second_url);
        assert_eq!(fourth_url, first_url);
    }

    #[tokio::test]
    async fn test_send_payload() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Create a test config with our mock server URL
        let config = Config::builder()
            .api_key("test_key")
            .project_id("test_project")
            .set_api_urls(vec![mock_server.uri()])
            .build()
            .unwrap();

        // Set up the mock response
        Mock::given(method("POST"))
            .and(header("x-api-key", "test_key"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = TreblleClient::new(config.clone()).unwrap();

        let payload = TrebllePayload {
            api_key: config.api_key,
            project_id: config.project_id,
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData::default(),
        };

        // Send the payload
        let result = client.send_to_treblle(payload).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        // Start a mock server that delays response
        let mock_server = MockServer::start().await;

        // Create config with mock server URL
        let config = Config::builder()
            .api_key("test_key")
            .project_id("test_project")
            .set_api_urls(vec![mock_server.uri()])
            .build()
            .unwrap();

        // Set up a mock that delays longer than our timeout
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(REQUEST_TIMEOUT + Duration::from_secs(1)),
            )
            .mount(&mock_server)
            .await;

        let client = TreblleClient::new(config.clone()).unwrap();

        let payload = TrebllePayload {
            api_key: config.api_key,
            project_id: config.project_id,
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData::default(),
        };

        let result = client.send_to_treblle(payload).await;
        assert!(matches!(result.unwrap_err(), TreblleError::Timeout));
    }
}
