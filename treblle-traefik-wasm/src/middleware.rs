use std::time::Instant;
use treblle_core::{extractors::TreblleExtractor, PayloadBuilder};

use crate::constants::host_features::{FEATURE_BUFFER_REQUEST, FEATURE_BUFFER_RESPONSE};
use crate::constants::http::{REQUEST_KIND, RESPONSE_KIND};
use crate::{
    extractors::WasmExtractor,
    host_functions,
    host_functions::headers::host_get_header_values,
    logger::{log, LogLevel},
    CONFIG, HTTP_CLIENT,
};

/// WASM middleware for Traefik that sends API analytics to Treblle
pub struct TreblleMiddleware;

impl TreblleMiddleware {
    /// Check if the request/response should be processed
    fn should_process(kind: u32) -> bool {
        host_get_header_values(kind, "content-type")
            .map(|ct| {
                let is_json = ct.to_lowercase().contains("application/json");
                log(LogLevel::Debug, &format!("Content-Type: {ct}, is_json: {is_json}"));
                is_json
            })
            .unwrap_or_else(|e| {
                log(LogLevel::Error, &format!("Failed to get Content-Type header: {e}"));
                false
            })
    }

    /// Process an incoming HTTP request
    pub fn handle_request() -> i64 {
        log(LogLevel::Debug, "Starting request processing");
        let start = Instant::now();

        if CONFIG.buffer_request {
            match host_functions::host_enable_features(FEATURE_BUFFER_REQUEST) {
                Ok(features) => {
                    log(LogLevel::Info, &format!("Enabled features: {features}"));
                }
                Err(e) => {
                    log(LogLevel::Error, &format!("Failed to enable request buffering: {e}"));
                    return 1;
                }
            }
        }

        // Check if we should process this request
        if !Self::should_process(REQUEST_KIND) {
            log(LogLevel::Debug, "Not a JSON request, skipping processing");
            return 1;
        }

        log(LogLevel::Debug, "Request is JSON, proceeding with processing");

        // Extract request data and check routing
        let start_extract = Instant::now();
        let request_payload =
            PayloadBuilder::build_request_payload::<WasmExtractor>(&(), &CONFIG.core);
        // Track timing for each stage
        log(
            LogLevel::Debug,
            &format!("Extracted request payload for URL: {}", request_payload.data.request.url),
        );
        log(LogLevel::Debug, &format!("Payload extraction took: {:?}", start_extract.elapsed()));

        // Check if route should be ignored
        if CONFIG.core.should_ignore_route(&request_payload.data.request.url) {
            log(LogLevel::Debug, &format!("Ignoring route: {}", request_payload.data.request.url));
            return 1;
        }

        // Send to Treblle using static HTTP client
        let start_send = Instant::now();
        match serde_json::to_vec(&request_payload) {
            Ok(payload_json) => {
                log(
                    LogLevel::Debug,
                    &format!(
                        "Sending request payload of size {} bytes to Treblle",
                        payload_json.len()
                    ),
                );
                log(
                    LogLevel::Debug,
                    &format!(
                        "JSON serialization took: {:?}, size: {} bytes",
                        start_send.elapsed(),
                        payload_json.len()
                    ),
                );
                if let Err(e) = HTTP_CLIENT.send(&payload_json, &CONFIG.core.api_key) {
                    log(LogLevel::Error, &format!("Failed to send request data to Treblle: {e}"));
                } else {
                    log(LogLevel::Debug, "Successfully sent request data to Treblle");
                }
            }
            Err(e) => log(LogLevel::Error, &format!("Failed to serialize request payload: {e}")),
        }

        log(LogLevel::Debug, &format!("Total request processing took: {:?}", start.elapsed()));

        1
    }

    /// Process an HTTP response
    pub fn handle_response(_req_ctx: i32, is_error: i32) {
        log(LogLevel::Debug, "Starting response processing");
        let start = Instant::now();

        if CONFIG.buffer_response {
            match host_functions::host_enable_features(FEATURE_BUFFER_RESPONSE) {
                Ok(features) => {
                    log(LogLevel::Info, &format!("Enabled features: {features}"));
                }
                Err(e) => {
                    log(LogLevel::Error, &format!("Failed to enable response buffering: {e}"));
                    return;
                }
            }
        }

        // Check if we should process this response
        if !Self::should_process(RESPONSE_KIND) {
            log(LogLevel::Debug, "Not a JSON response, skipping processing");
            return;
        }

        log(
            LogLevel::Debug,
            "Response is JSON and buffering is enabled, proceeding with processing",
        );

        let start_time = Instant::now();

        // Extract response data
        let start_extract = Instant::now();
        let mut response_payload = PayloadBuilder::build_response_payload::<WasmExtractor>(
            &(),
            &CONFIG.core,
            start_time.elapsed(),
        );
        log(
            LogLevel::Debug,
            &format!("Extracted response payload for URL: {}", response_payload.data.request.url),
        );
        log(LogLevel::Debug, &format!("Payload extraction took: {:?}", start_extract.elapsed()));

        // Add error information if needed
        if is_error != 0 || response_payload.data.response.code >= 400 {
            if let Some(errors) = WasmExtractor::extract_error_info(&()) {
                response_payload.data.errors.extend(errors);
            }
        }

        // Send to Treblle using static HTTP client
        let start_send = Instant::now();
        match serde_json::to_vec(&response_payload) {
            Ok(payload_json) => {
                log(
                    LogLevel::Debug,
                    &format!(
                        "Sending response payload of size {} bytes to Treblle",
                        payload_json.len()
                    ),
                );
                log(
                    LogLevel::Debug,
                    &format!(
                        "JSON serialization took: {:?}, size: {} bytes",
                        start_send.elapsed(),
                        payload_json.len()
                    ),
                );
                if let Err(e) = HTTP_CLIENT.send(&payload_json, &CONFIG.core.api_key) {
                    log(LogLevel::Error, &format!("Failed to send response data to Treblle: {e}"));
                } else {
                    log(LogLevel::Debug, "Successfully sent response data to Treblle");
                }
            }
            Err(e) => log(LogLevel::Error, &format!("Failed to serialize response payload: {e}")),
        }

        log(LogLevel::Debug, &format!("Total response processing took: {:?}", start.elapsed()));
    }
}
