//! WASM middleware implementation for Treblle Traefik plugin.
//!
//! This module provides the main entry point for the WASM middleware,
//! handling request/response processing and communication with Treblle API.

use once_cell::sync::Lazy;
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
mod bindings;
#[cfg(target_arch = "wasm32")]
mod certs;
mod config;
mod constants;
mod extractors;
mod host_functions;
#[cfg(target_arch = "wasm32")]
mod wasi_http_client;

use crate::constants::http::{REQUEST_KIND, RESPONSE_KIND};
#[cfg(target_arch = "wasm32")]
use crate::constants::log_level;
#[cfg(target_arch = "wasm32")]
use crate::extractors::WasmExtractor;
#[cfg(target_arch = "wasm32")]
use bindings::exports::traefik::http_handler::handler::Guest;
use config::WasmConfig;
#[cfg(target_arch = "wasm32")]
use treblle_core::error::Result as TreblleResult;
#[cfg(target_arch = "wasm32")]
use treblle_core::schema::TrebllePayload;
#[cfg(target_arch = "wasm32")]
use treblle_core::{PayloadBuilder, TreblleError};

/// Global configuration instance
static CONFIG: Lazy<WasmConfig> = Lazy::new(|| {
    let config_json = host_functions::host_get_config().expect("Failed to get config from host");
    WasmConfig::from_json(&config_json).expect("Failed to parse config")
});

/// Global HTTP client instance
#[cfg(target_arch = "wasm32")]
static HTTP_CLIENT: Lazy<Mutex<wasi_http_client::WasiHttpClient>> = Lazy::new(|| {
    Mutex::new(wasi_http_client::WasiHttpClient::new(
        CONFIG.core.api_urls.clone(),
    ))
});

/// Main handler for WASM middleware
#[cfg(target_arch = "wasm32")]
pub struct HttpHandler;

#[cfg(target_arch = "wasm32")]
impl Guest for HttpHandler {
    /// Handle an incoming HTTP request
    ///
    /// This function is called by the Traefik middleware to process an incoming HTTP request.
    /// It checks if the request should be processed based on config rules and sends request
    /// data to Treblle if appropriate.
    ///
    /// # Returns
    ///
    /// Returns 1 to indicate that Traefik should continue processing the request.
    fn handle_request() -> i64 {
        host_functions::host_log(log_level::DEBUG, "Handling request in WASM module");

        if should_process(REQUEST_KIND) {
            if CONFIG.buffer_response {
                let features = host_functions::host_enable_features(2); // Enable FeatureBufferResponse
                host_functions::host_log(log_level::INFO, &format!("Enabled features: {features}"));
            }

            let request_payload =
                PayloadBuilder::build_request_payload::<WasmExtractor>(&(), &CONFIG.core);

            // Send request payload to Treblle
            if let Err(e) = send_to_treblle(request_payload) {
                host_functions::host_log(
                    log_level::ERROR,
                    &format!("Failed to send request payload: {e}"),
                );
            }
        }

        1 // Always continue processing the request
    }

    /// Handle an HTTP response
    ///
    /// This function is called by the Traefik middleware to process an HTTP response.
    ///
    /// # Arguments
    ///
    /// * `req_ctx` - The request context
    /// * `is_error` - Indicates if the response is an error
    fn handle_response(_req_ctx: i32, _is_error: i32) {
        host_functions::host_log(log_level::DEBUG, "Handling response in WASM module");

        if should_process(RESPONSE_KIND) {
            let duration = std::time::Duration::from_secs(0); // TODO: Implement proper duration tracking
            let response_payload = PayloadBuilder::build_response_payload::<WasmExtractor>(
                &(),
                &CONFIG.core,
                duration,
            );

            if let Err(e) = send_to_treblle(response_payload) {
                host_functions::host_log(
                    log_level::ERROR,
                    &format!("Failed to send response payload: {e}"),
                );
            }
        }

        host_functions::host_log(log_level::DEBUG, "Finished processing response");
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn handle_request() -> i64 {
    HttpHandler::handle_request()
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn handle_response(req_ctx: i32, is_error: i32) {
    HttpHandler::handle_response(req_ctx, is_error)
}

/// Determines if a request/response should be processed based on config rules
fn should_process(kind: u32) -> bool {
    let uri = host_functions::host_get_uri().unwrap_or_default();

    // Check both exact matches and regex patterns for ignored routes
    if CONFIG.core.ignored_routes.contains(&uri) {
        return false;
    }

    if CONFIG
        .core
        .ignored_routes_regex
        .iter()
        .any(|re| re.is_match(&uri))
    {
        return false;
    }

    // Check content type
    host_functions::host_get_header_values(kind, "Content-Type")
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false)
}

/// Sends payload to Treblle API
#[cfg(target_arch = "wasm32")]
fn send_to_treblle(payload: TrebllePayload) -> TreblleResult<()> {
    let http_client = HTTP_CLIENT.lock().map_err(|e| {
        host_functions::host_log(
            log_level::ERROR,
            &format!("Failed to acquire HTTP_CLIENT lock: {e}"),
        );
        TreblleError::LockError(e.to_string())
    })?;

    let payload_json = payload.to_json()?;

    http_client
        .post(payload_json.as_bytes(), &CONFIG.core.api_key)
        .map_err(|e| {
            host_functions::host_log(
                log_level::ERROR,
                &format!("Failed to send data to Treblle API: {e}"),
            );
            TreblleError::Http(format!("Failed to send data to Treblle API: {e}"))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_functions::test as host_test;
    use once_cell::sync::Lazy;
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub fn setup_test_config() {
        INIT.call_once(|| {
            let config_json = r#"{
                "apiKey": "test_key",
                "projectId": "test_project",
                "ignoredRoutes": ["/health"],
                "ignoredRoutesRegex": ["^/internal/.*$"],
                "bufferResponse": true,
                "rootCaPath": null,
                "logLevel": 0
            }"#;
            host_test::setup_config(config_json);
        });
    }

    #[test]
    fn test_should_process() {
        setup_test_config();

        // Set up test config
        let config_json = r#"{
            "apiKey": "test_key",
            "projectId": "test_project",
            "ignoredRoutes": ["/health"],
            "ignoredRoutesRegex": ["^/internal/.*$"],
            "bufferResponse": false
        }"#;
        host_test::setup_config(config_json);

        // Test ignored exact match
        host_test::setup_request(
            "GET",
            "/health",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            vec![],
        );
        assert!(!should_process(REQUEST_KIND));

        // Test ignored regex match
        host_test::setup_request(
            "GET",
            "/internal/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            vec![],
        );
        assert!(!should_process(REQUEST_KIND));

        // Test non-JSON content type
        host_test::setup_request(
            "GET",
            "/api/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "text/plain".to_string())],
            vec![],
        );
        assert!(!should_process(REQUEST_KIND));

        // Test valid request
        host_test::setup_request(
            "POST",
            "/api/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            vec![],
        );
        assert!(should_process(REQUEST_KIND));
    }
}
