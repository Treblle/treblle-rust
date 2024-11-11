use std::sync::Arc;
use std::time::Instant;

use once_cell::sync::OnceCell;
use treblle_core::extractors::TreblleExtractor;
use treblle_core::PayloadBuilder;

use crate::constants::http::{REQUEST_KIND, RESPONSE_KIND};
use crate::{
    config::WasmConfig,
    extractors::WasmExtractor,
    host_functions::{host_enable_features, host_read_body, host_write_body},
    log_debug, log_error,
    wasi_http_client::WasiHttpClient,
};

const FEATURE_BUFFER_RESPONSE: u32 = 2;

static CONFIG: OnceCell<WasmConfig> = OnceCell::new();
static HTTP_CLIENT: OnceCell<Arc<WasiHttpClient>> = OnceCell::new();

/// WASM middleware for Traefik that sends API analytics to Treblle
pub struct TreblleMiddleware;

impl TreblleMiddleware {
    /// Initialize the middleware with configuration
    pub fn init() {
        // Initialize configuration
        let config = WasmConfig::get_or_fallback();

        log_debug!("Initializing Treblle middleware with config: {:?}", config);

        // Initialize HTTP client
        let http_client = Arc::new(WasiHttpClient::new(
            config.core.api_urls.clone(),
            config.max_retries,
            config.max_pool_size,
        ));

        // Store in global state
        CONFIG.get_or_init(|| config);
        HTTP_CLIENT.get_or_init(|| http_client);

        // Enable response buffering if configured
        if CONFIG.get().unwrap().buffer_response {
            if let Ok(features) = host_enable_features(FEATURE_BUFFER_RESPONSE) {
                log_debug!("Enabled features: {}", features);
            }
        }
    }

    /// Process an incoming HTTP request
    pub fn handle_request() -> i64 {
        let start_time = Instant::now();
        let config = match CONFIG.get() {
            Some(config) => config,
            None => {
                log_error!("Middleware not initialized");
                return 1;
            }
        };

        if let Err(e) = Self::process_request(config, start_time) {
            log_error!("Error processing request: {}", e);
        }

        // Always continue processing the request
        1
    }

    /// Process an HTTP response
    pub fn handle_response(req_ctx: i32, is_error: i32) {
        let config = match CONFIG.get() {
            Some(config) => config,
            None => {
                log_error!("Middleware not initialized");
                return;
            }
        };

        if !config.buffer_response {
            log_debug!("Response processing disabled");
            return;
        }

        if let Err(e) = Self::process_response(config, req_ctx, is_error) {
            log_error!("Error processing response: {}", e);
        }
    }

    fn process_request(config: &WasmConfig, _start_time: Instant) -> treblle_core::Result<()> {
        // Read raw body first, before any other processing
        match host_read_body(REQUEST_KIND) {
            Ok(body) => {
                // Store the body for later use
                WasmExtractor::store_request_body(body.clone());

                // Write body back to ensure it's available for the next middleware
                if let Err(e) = host_write_body(REQUEST_KIND, &body) {
                    log_error!("Failed to write back request body: {}", e);
                }
            }
            Err(e) => {
                log_error!("Failed to read request body: {}", e);
            }
        }

        // Now extract request data using the WasmExtractor
        let mut payload = PayloadBuilder::build_request_payload::<WasmExtractor>(&(), &config.core);

        // Check if route should be ignored
        if config.core.should_ignore_route(&payload.data.request.url) {
            log_debug!("Ignoring route: {}", payload.data.request.url);
            return Ok(());
        }

        // Send data to Treblle API asynchronously
        if let Some(client) = HTTP_CLIENT.get() {
            if let Ok(payload_json) = serde_json::to_vec(&payload) {
                if let Err(e) = client.send(&payload_json, &config.core.api_key) {
                    log_error!("Failed to send request data to Treblle: {}", e);
                }
            }
        }

        Ok(())
    }

    fn process_response(
        config: &WasmConfig,
        _req_ctx: i32,
        is_error: i32,
    ) -> treblle_core::Result<()> {
        let start_time = Instant::now();

        // Read raw body first, before any other processing
        match host_read_body(RESPONSE_KIND) {
            Ok(body) => {
                // Store the body for later use
                WasmExtractor::store_response_body(body.clone());

                // Write body back to ensure it's available for the next middleware
                if let Err(e) = host_write_body(RESPONSE_KIND, &body) {
                    log_error!("Failed to write back response body: {}", e);
                }
            }
            Err(e) => {
                log_error!("Failed to read response body: {}", e);
            }
        }

        // Extract response data using the WasmExtractor
        let mut payload = PayloadBuilder::build_response_payload::<WasmExtractor>(
            &(),
            &config.core,
            start_time.elapsed(),
        );

        // Add error information if needed
        if is_error != 0 || payload.data.response.code >= 400 {
            if let Some(errors) = WasmExtractor::extract_error_info(&()) {
                for error in errors {
                    payload.data.errors.push(error);
                }
            }
        }

        // Send data to Treblle API asynchronously
        if let Some(client) = HTTP_CLIENT.get() {
            if let Ok(payload_json) = serde_json::to_vec(&payload) {
                if let Err(e) = client.send(&payload_json, &config.core.api_key) {
                    log_error!("Failed to send response data to Treblle: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LogLevel;

    #[test]
    fn test_middleware_initialization() {
        // Create a test config
        let config = WasmConfig {
            core: treblle_core::Config::new("test_key".into(), "test_project".into()),
            buffer_response: true,
            root_ca_path: None,
            max_retries: 3,
            max_pool_size: 10,
            log_level: LogLevel::Debug,
        };

        // Initialize globals
        CONFIG.get_or_init(|| config);
        let http_client = Arc::new(WasiHttpClient::new(
            vec!["https://api.treblle.com".to_string()],
            3,
            10,
        ));
        HTTP_CLIENT.get_or_init(|| http_client);

        assert!(CONFIG.get().is_some());
        assert!(HTTP_CLIENT.get().is_some());

        // Verify config values
        let stored_config = CONFIG.get().unwrap();
        assert_eq!(stored_config.max_retries, 3);
        assert_eq!(stored_config.max_pool_size, 10);
        assert_eq!(stored_config.log_level, LogLevel::Debug);
    }
}