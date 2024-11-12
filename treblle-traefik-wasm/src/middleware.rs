use std::time::Instant;

use treblle_core::extractors::TreblleExtractor;
use treblle_core::PayloadBuilder;

use crate::{
    constants::http::{REQUEST_KIND, RESPONSE_KIND},
    extractors::WasmExtractor,
    host_functions::{host_enable_features, host_read_body, host_write_body},
    logger::{log, LogLevel},
    wasi_http_client::WasiHttpClient,
    CONFIG, HTTP_CLIENT,
};

const FEATURE_BUFFER_RESPONSE: u32 = 2;

/// WASM middleware for Traefik that sends API analytics to Treblle
pub struct TreblleMiddleware;

impl TreblleMiddleware {
    /// Initialize the middleware with configuration
    pub fn init() {
        log(LogLevel::Debug, "Starting Treblle middleware initialization");

        // Initialize HTTP client
        let http_client = std::sync::Arc::new(WasiHttpClient::new(
            CONFIG.core.api_urls.clone(),
            CONFIG.max_retries,
            CONFIG.max_pool_size,
            CONFIG.root_ca_path.clone(),
        ));

        // Store HTTP client in global state
        if let Err(_) = HTTP_CLIENT.set(http_client) {
            log(LogLevel::Error, "Failed to store HTTP client in global state");
            return;
        }

        // Enable response buffering if configured
        if CONFIG.buffer_response {
            let features = host_enable_features(FEATURE_BUFFER_RESPONSE);
            if features & FEATURE_BUFFER_RESPONSE != 0 {
                log(LogLevel::Debug, "Response buffering enabled");
            } else {
                log(LogLevel::Error, "Failed to enable response buffering");
            }
        }

        log(LogLevel::Info, "Treblle middleware initialized successfully with:");
        log(LogLevel::Debug, &format!("  API Key: {}", CONFIG.core.api_key));
        log(LogLevel::Debug, &format!("  Project ID: {}", CONFIG.core.project_id));
        log(LogLevel::Debug, &format!("  API URLs: {:?}", CONFIG.core.api_urls));
        log(LogLevel::Debug, &format!("  Buffer Response: {}", CONFIG.buffer_response));
        log(LogLevel::Debug, &format!("  Log Level: {:?}", CONFIG.log_level));
    }

    /// Process an incoming HTTP request
    pub fn handle_request() -> i64 {
        let start_time = Instant::now();

        if let Err(e) = Self::process_request(start_time) {
            log(LogLevel::Error, &format!("Error processing request: {}", e));
        }

        // Always continue processing the request
        1
    }

    /// Process an HTTP response
    pub fn handle_response(req_ctx: i32, is_error: i32) {
        if !CONFIG.buffer_response {
            log(LogLevel::Debug, "Response processing disabled");
            return;
        }

        if let Err(e) = Self::process_response(req_ctx, is_error) {
            log(LogLevel::Error, &format!("Error processing response: {}", e));
        }
    }

    fn process_request(_start_time: Instant) -> treblle_core::Result<()> {
        // Read raw body first
        match host_read_body(REQUEST_KIND) {
            Ok(body) => {
                WasmExtractor::store_request_body(body.clone());
                if let Err(e) = host_write_body(REQUEST_KIND, &body) {
                    log(LogLevel::Error, &format!("Failed to write back request body: {}", e));
                }
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Failed to read request body: {}", e));
            }
        }

        // Extract request data
        let payload = PayloadBuilder::build_request_payload::<WasmExtractor>(&(), &CONFIG.core);

        // Check if route should be ignored
        if CONFIG.core.should_ignore_route(&payload.data.request.url) {
            log(LogLevel::Debug, &format!("Ignoring route: {}", payload.data.request.url));
            return Ok(());
        }

        // Send data to Treblle API
        if let Some(client) = HTTP_CLIENT.get() {
            if let Ok(payload_json) = serde_json::to_vec(&payload) {
                if let Err(e) = client.send(&payload_json, &CONFIG.core.api_key) {
                    log(LogLevel::Error, &format!("Failed to send request data to Treblle: {}", e));
                }
            }
        }

        Ok(())
    }

    fn process_response(_req_ctx: i32, is_error: i32) -> treblle_core::Result<()> {
        let start_time = Instant::now();

        // Read raw body
        match host_read_body(RESPONSE_KIND) {
            Ok(body) => {
                WasmExtractor::store_response_body(body.clone());
                if let Err(e) = host_write_body(RESPONSE_KIND, &body) {
                    log(LogLevel::Error, &format!("Failed to write back response body: {}", e));
                }
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Failed to read response body: {}", e));
            }
        }

        // Extract response data
        let mut payload = PayloadBuilder::build_response_payload::<WasmExtractor>(
            &(),
            &CONFIG.core,
            start_time.elapsed(),
        );

        // Add error information if needed
        if is_error != 0 || payload.data.response.code >= 400 {
            if let Some(errors) = WasmExtractor::extract_error_info(&()) {
                payload.data.errors.extend(errors);
            }
        }

        // Send data to Treblle API
        if let Some(client) = HTTP_CLIENT.get() {
            if let Ok(payload_json) = serde_json::to_vec(&payload) {
                if let Err(e) = client.send(&payload_json, &CONFIG.core.api_key) {
                    log(
                        LogLevel::Error,
                        &format!("Failed to send response data to Treblle: {}", e),
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WasmConfig;
    use treblle_core::Config as CoreConfig;

    #[test]
    fn test_middleware_initialization() {
        // Initialize globals
        let http_client = std::sync::Arc::new(WasiHttpClient::new(
            vec!["https://api.treblle.com".to_string()],
            3,
            10,
            None,
        ));

        HTTP_CLIENT.set(http_client).unwrap();

        // Verify HTTP client is set
        assert!(HTTP_CLIENT.get().is_some());
    }
}
