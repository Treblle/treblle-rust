//! Treblle middleware for Traefik
//!
//! This crate implements a WASM-based middleware for Traefik that integrates
//! with Treblle's API monitoring and logging services.

#![cfg_attr(test, allow(unused_imports, dead_code))]

pub mod bindings;
pub mod certs;
pub mod config;
pub mod extractors;
pub mod host_functions;
pub mod middleware;
pub mod wasi_http_client;
mod macros;
mod constants;

use bindings::exports::traefik::http_handler::handler::Guest;
use middleware::TreblleMiddleware;

/// Implements the Traefik HTTP handler interface
impl Guest for TreblleMiddleware {
    /// Handle an incoming HTTP request
    fn handle_request() -> i64 {
        TreblleMiddleware::handle_request()
    }

    /// Handle an HTTP response
    fn handle_response(req_ctx: i32, is_error: i32) {
        TreblleMiddleware::handle_response(req_ctx, is_error)
    }
}

#[no_mangle]
pub extern "C" fn init() {
    TreblleMiddleware::init();
}

#[no_mangle]
pub extern "C" fn handle_request() -> i64 {
    TreblleMiddleware::handle_request()
}

#[no_mangle]
pub extern "C" fn handle_response(req_ctx: i32, is_error: i32) {
    TreblleMiddleware::handle_response(req_ctx, is_error)
}