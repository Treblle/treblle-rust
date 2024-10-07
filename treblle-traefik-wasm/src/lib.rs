use once_cell::sync::Lazy;
use std::sync::Mutex;

mod bindings;
mod certs;
mod config;
mod constants;
mod host_functions;
mod http_handler;
mod wasi_http_client;

use treblle_core::error::Result as TreblleResult;
use treblle_core::payload::mask_payload;
use treblle_core::schema::TrebllePayload;
use treblle_core::TreblleError;

use crate::constants::log_level;
use crate::http_handler::WasmExtractor;
use bindings::exports::traefik::http_handler::handler::Guest;
use config::WasmConfig;

static CONFIG: Lazy<WasmConfig> = Lazy::new(|| {
    let config_json = host_functions::host_get_config().expect("Failed to get config from host");
    WasmConfig::from_json(&config_json).expect("Failed to parse config")
});

static HTTP_CLIENT: Lazy<Mutex<wasi_http_client::WasiHttpClient>> = Lazy::new(|| {
    Mutex::new(wasi_http_client::WasiHttpClient::new(
        CONFIG.core.api_urls.clone(),
    ))
});

pub struct HttpHandler;

impl Guest for HttpHandler {
    /// Handle an incoming HTTP request
    ///
    /// This function is called by the Traefik middleware to process an incoming HTTP request.
    ///
    /// # Returns
    ///
    /// Returns 1 to indicate that Traefik should continue processing the request.
    fn handle_request() -> i64 {
        host_functions::host_log(log_level::DEBUG, "Handling request in WASM module");

        let uri = host_functions::host_get_uri().unwrap_or_default();
        let should_process = !CONFIG
            .core
            .ignored_routes
            .iter()
            .any(|route| uri.starts_with(route));

        if should_process {
            if CONFIG.buffer_response {
                let features = host_functions::host_enable_features(2); // Enable FeatureBufferResponse
                host_functions::host_log(log_level::INFO, &format!("Enabled features: {features}"));
            }

            let mut request_payload = WasmExtractor::build_request_payload(
                CONFIG.core.api_key.clone(),
                CONFIG.core.project_id.clone(),
            );
            mask_payload(&mut request_payload, &CONFIG.core.masked_fields);

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

        let uri = host_functions::host_get_uri().unwrap_or_default();
        let should_process = !CONFIG
            .core
            .ignored_routes
            .iter()
            .any(|route| uri.starts_with(route));

        if should_process && CONFIG.buffer_response {
            let start_time = std::time::Instant::now();
            let mut response_payload = WasmExtractor::build_response_payload(
                CONFIG.core.api_key.clone(),
                CONFIG.core.project_id.clone(),
                start_time,
            );
            mask_payload(&mut response_payload, &CONFIG.core.masked_fields);

            // Send response payload to Treblle
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



#[no_mangle]
pub extern "C" fn handle_request() -> i64 {
    HttpHandler::handle_request()
}

#[no_mangle]
pub extern "C" fn handle_response(req_ctx: i32, is_error: i32) {
    HttpHandler::handle_response(req_ctx, is_error)
}

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
