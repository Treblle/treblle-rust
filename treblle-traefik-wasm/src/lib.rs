//! Treblle middleware for Traefik
//!
//! This crate implements a WASM-based middleware for Traefik that integrates
//! with Treblle's API monitoring and logging services.

#![cfg_attr(test, allow(unused_imports, dead_code))]

pub mod bindings;
pub mod certs;
pub mod config;
pub mod constants;
pub mod extractors;
pub mod host_functions;
pub mod logger;
pub mod middleware;
pub mod wasi_http_client;

use std::sync::Arc;

use bindings::exports::traefik::http_handler::handler::Guest;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;

use crate::config::WasmConfig;
use crate::logger::{log, LogLevel};
use crate::middleware::TreblleMiddleware;
use crate::wasi_http_client::WasiHttpClient;

// Global configuration and HTTP client
pub static CONFIG: Lazy<WasmConfig> = Lazy::new(|| WasmConfig::builder().build().unwrap());
pub static HTTP_CLIENT: OnceCell<Arc<WasiHttpClient>> = OnceCell::new();

/// Implements the Traefik HTTP handler interface
impl Guest for TreblleMiddleware {
    /// Handle an incoming HTTP request
    fn handle_request() -> i64 {
        // Initialize logging and force lazy statics
        logger::init();
        Lazy::force(&CONFIG);

        log(LogLevel::Debug, "Handling request in WASM module");

        if CONFIG.buffer_response {
            let features = host_functions::host_enable_features(2); // Enable FeatureBufferResponse
            log(LogLevel::Info, &format!("Enabled features: {features}"));
        }

        TreblleMiddleware::handle_request()
    }

    /// Handle an HTTP response
    fn handle_response(req_ctx: i32, is_error: i32) {
        // Initialize logging
        logger::init();

        log(LogLevel::Debug, "Handling response in WASM module");
        TreblleMiddleware::handle_response(req_ctx, is_error);
        log(LogLevel::Debug, "Finished processing response");
    }
}

#[no_mangle]
pub extern "C" fn init() {
    logger::init();
    log(LogLevel::Debug, "Initializing Treblle middleware");
    TreblleMiddleware::init();
    log(LogLevel::Info, "Treblle middleware initialized");
}

#[no_mangle]
pub extern "C" fn handle_request() -> i64 {
    TreblleMiddleware::handle_request()
}

#[no_mangle]
pub extern "C" fn handle_response(req_ctx: i32, is_error: i32) {
    TreblleMiddleware::handle_response(req_ctx, is_error);
}
