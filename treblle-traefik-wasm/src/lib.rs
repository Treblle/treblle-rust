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

use once_cell::sync::Lazy;

use crate::config::WasmConfig;
use crate::logger::{log, LogLevel};
use crate::middleware::TreblleMiddleware;
use crate::wasi_http_client::WasiHttpClient;

use bindings::exports::traefik::http_handler::handler::Guest;

pub static CONFIG: Lazy<WasmConfig> = Lazy::new(|| {
    WasmConfig::get_or_fallback().unwrap_or_else(|e| {
        log(LogLevel::Error, &format!("Failed to get config from host, using defaults: {}", e));
        WasmConfig::builder().api_key("default").build().expect("Failed to create default config")
    })
});

// Initialize HTTP client statically
pub static HTTP_CLIENT: Lazy<Arc<WasiHttpClient>> = Lazy::new(|| {
    log(LogLevel::Debug, "Initializing HTTP client");

    // Log the configuration being used
    log(LogLevel::Debug, &format!("Using config: {:?}", &*CONFIG));

    let client = WasiHttpClient::new(
        CONFIG.core.api_urls.clone(),
        CONFIG.max_retries,
        CONFIG.max_pool_size,
        CONFIG.root_ca_path.clone(),
    );

    log(LogLevel::Debug, "HTTP client initialized successfully");
    Arc::new(client)
});

// Implement the Guest trait required by Traefik
impl Guest for TreblleMiddleware {
    fn handle_request() -> i64 {
        // Force initialization of our statics if not already done
        Lazy::force(&CONFIG);
        Lazy::force(&HTTP_CLIENT);

        log(LogLevel::Debug, "Guest::handle_request called");
        TreblleMiddleware::handle_request()
    }

    fn handle_response(req_ctx: i32, is_error: i32) {
        log(LogLevel::Debug, "Guest::handle_response called");
        TreblleMiddleware::handle_response(req_ctx, is_error);
    }
}

// No-mangle functions required for WASM export
#[no_mangle]
pub extern "C" fn handle_request() -> i64 {
    <TreblleMiddleware as Guest>::handle_request()
}

#[no_mangle]
pub extern "C" fn handle_response(req_ctx: i32, is_error: i32) {
    <TreblleMiddleware as Guest>::handle_response(req_ctx, is_error);
}
