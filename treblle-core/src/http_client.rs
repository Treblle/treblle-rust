use crate::constants::http::TIMEOUT_SECONDS;
use crate::error::{Result as TreblleResult, TreblleError};
use crate::schema::TrebllePayload;
use crate::Config;
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// First, implement From<reqwest::Error> for TreblleError
impl From<reqwest::Error> for TreblleError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            TreblleError::Timeout
        } else if err.is_connect() {
            TreblleError::Http(format!("Connection error: {}", err))
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
        let client = Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECONDS))
            .build()
            .map_err(|e| TreblleError::Http(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            config,
            current_url_index: AtomicUsize::new(0),
        })
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
    use tokio;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_client_rotation() {
        let config = Config::new("test_key".to_string(), "test_project".to_string());
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
        let mut config = Config::new("test_key".to_string(), "test_project".to_string());
        config.api_urls = vec![mock_server.uri()];

        // Set up the mock response
        Mock::given(method("POST"))
            .and(header("x-api-key", "test_key"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = TreblleClient::new(config).unwrap();
        let payload = TrebllePayload::new("test_key".to_string(), "test_project".to_string());

        // Send the payload
        let result = client.send_to_treblle(payload).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        // Start a mock server that delays response
        let mock_server = MockServer::start().await;

        let mut config = Config::new("test_key".to_string(), "test_project".to_string());
        config.api_urls = vec![mock_server.uri()];

        // Set up a mock that delays longer than our timeout
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(Duration::from_secs(TIMEOUT_SECONDS + 1)),
            )
            .mount(&mock_server)
            .await;

        let client = TreblleClient::new(config).unwrap();
        let payload = TrebllePayload::new("test_key".to_string(), "test_project".to_string());

        let result = client.send_to_treblle(payload).await;
        assert!(matches!(result.unwrap_err(), TreblleError::Timeout));
    }
}
