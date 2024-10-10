use once_cell::sync::Lazy;
use std::sync::Mutex;

mod bindings;
mod certs;
mod config;
mod constants;
mod host_functions;
mod wasi_http_client;
mod extractors;

use treblle_core::error::Result as TreblleResult;
use treblle_core::schema::TrebllePayload;
use treblle_core::{PayloadBuilder, TreblleError};

use crate::constants::log_level;
use bindings::exports::traefik::http_handler::handler::Guest;
use config::WasmConfig;
use crate::constants::http::{REQUEST_KIND, RESPONSE_KIND};
use crate::extractors::WasmExtractor;

static CONFIG: Lazy<WasmConfig> = Lazy::new(|| {
    let config_json = host_functions::host_get_config()
        .expect("Failed to get config from host");
    WasmConfig::from_json(&config_json)
        .expect("Failed to parse config")
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

        if should_process(REQUEST_KIND) {
            if CONFIG.buffer_response {
                let features = host_functions::host_enable_features(2); // Enable FeatureBufferResponse
                host_functions::host_log(log_level::INFO, &format!("Enabled features: {features}"));
            }

            let request_payload = PayloadBuilder::build_request_payload::<WasmExtractor>(
                &(),
                &CONFIG.core
            );

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
            let response_payload = PayloadBuilder::build_response_payload::<WasmExtractor>(&(), &CONFIG.core, duration);

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

fn should_process(kind: u32) -> bool {
    !CONFIG.core.ignored_routes.iter().any(|route| {
        route.is_match(&host_functions::host_get_uri().unwrap_or_default())
    }) && host_functions::host_get_header_values(kind, "Content-Type")
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false)
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
