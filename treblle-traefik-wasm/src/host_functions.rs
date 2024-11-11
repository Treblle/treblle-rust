//! Host functions module for the Treblle middleware.
//!
//! This module provides the interface between the WebAssembly module
//! and the host environment (Traefik).

use core::str;
use std::ffi::CString;
use treblle_core::{Result, TreblleError};

// Defines the external functions provided by the host environment.
// External functions come from the `http_handler` module exposed by `http-wasm-host-go/api/handler`
// - https://github.com/http-wasm/http-wasm-host-go/blob/main/api/handler/handler.go
// - https://github.com/http-wasm/http-wasm-host-go/blob/main/api/handler/wasm.go
// - https://github.com/http-wasm/http-wasm/blob/main/content/http-handler-abi.md
#[cfg(feature = "wasm")]
#[link(wasm_import_module = "http_handler")]
extern "C" {
    fn log(level: i32, message: *const u8, message_len: u32);
    fn enable_features(features: u32) -> u32;
    fn get_config(buf: *mut u8, buf_limit: i32) -> i32;
    fn get_method(buf: *mut u8, buf_limit: i32) -> i32;
    fn get_uri(ptr: *mut u8, buf_limit: u32) -> i32;
    fn get_protocol_version(buf: *mut u8, buf_limit: i32) -> i32;
    fn get_header_names(header_kind: u32, buf: *mut u8, buf_limit: i32) -> i64;
    fn get_header_values(
        header_kind: u32,
        name_ptr: *const u8,
        name_len: u32,
        buf: *mut u8,
        buf_limit: i32,
    ) -> i64;
    fn read_body(body_kind: u32, ptr: *mut u8, buf_limit: u32) -> i64;
    fn write_body(body_kind: u32, ptr: *const u8, message_len: u32);

    fn get_status_code() -> u32;
}

/// Logs a message to the host environment.
///
/// # Arguments
///
/// * `level` - The log level (use constants from `crate::constants`).
/// * `message` - The message to log.
#[cfg(feature = "wasm")]
pub fn host_log(level: i32, message: &str) {
    let sanitized_message = message.replace('\0', "");

    if let Ok(c_message) = CString::new(sanitized_message) {
        unsafe {
            log(level, c_message.as_ptr() as *const u8, c_message.as_bytes().len() as u32);
        }
    }
}

/// Enables features in the host environment.
///
/// # Arguments
///
/// * `features` - A bitfield of features to enable.
///
/// # Returns
///
/// Returns a bitfield of successfully enabled features.
#[cfg(feature = "wasm")]
pub fn host_enable_features(features: u32) -> u32 {
    unsafe { enable_features(features) }
}

/// Retrieves the configuration from the host environment.
///
/// # Returns
///
/// Returns the configuration as a string, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_config() -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe { get_config(buf, buf_limit) })
}

/// Retrieves the HTTP method of the current request.
///
/// # Returns
///
/// Returns the HTTP method as a string, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_method() -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe { get_method(buf, buf_limit) })
}

/// Retrieves the URI of the current request.
///
/// # Returns
///
/// Returns the URI as a string, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_uri() -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe { get_uri(buf, buf_limit as u32) })
}

/// Retrieves the protocol version of the current request.
///
/// # Returns
///
/// Returns the protocol version as a string, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_protocol_version() -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe { get_protocol_version(buf, buf_limit) })
}

/// Retrieves the header names for a given header kind.
///
/// # Arguments
///
/// * `header_kind` - The kind of headers to retrieve (0 for request, 1 for response).
///
/// # Returns
///
/// Returns a comma-separated string of header names, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_header_names(header_kind: u32) -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe {
        get_header_names(header_kind, buf, buf_limit) as i32
    })
}

/// Retrieves the values for a specific header.
///
/// # Arguments
///
/// * `header_kind` - The kind of headers to retrieve from (0 for request, 1 for response).
/// * `name` - The name of the header to retrieve values for.
///
/// # Returns
///
/// Returns a comma-separated string of header values, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_header_values(header_kind: u32, name: &str) -> Result<String> {
    let sanitized_name = name.replace('\0', "");
    let c_name = CString::new(sanitized_name)
        .map_err(|e| TreblleError::HostFunction(format!("Invalid header name: {}", e)))?;

    read_from_buffer(|buf, buf_limit| unsafe {
        get_header_values(
            header_kind,
            c_name.as_ptr() as *const u8,
            c_name.as_bytes().len() as u32,
            buf,
            buf_limit,
        ) as i32
    })
}

/// Reads the body of the current request or response.
///
/// # Arguments
///
/// * `body_kind` - The kind of body to read (0 for request, 1 for response).
///
/// # Returns
///
/// Returns the body as a vector of bytes, or an error if reading fails.
#[cfg(feature = "wasm")]
pub fn host_read_body(body_kind: u32) -> Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(4096);
    let read = unsafe { read_body(body_kind, buffer.as_mut_ptr(), 4096) };

    if read < 0 {
        Err(TreblleError::HostFunction("Error reading body".to_string()))
    } else {
        unsafe {
            buffer.set_len(read as usize);
        }
        Ok(buffer)
    }
}

/// Writes the body back to ensure the original request body is available for
/// the rest of the request processing pipeline.
///
/// # Arguments
///
/// * `body_kind` - The kind of body to read (0 for request, 1 for response).
/// * `body`: - Original body.
///
/// # Returns
///
/// Returns Result<(), TreblleError>
pub fn host_write_body(body_kind: u32, body: &[u8]) -> Result<()> {
    unsafe {
        write_body(body_kind, body.as_ptr(), body.len() as u32);
    }

    Ok(())
}

/// Retrieves the status code of the current response.
///
/// # Returns
///
/// Returns the status code as an u32.
#[cfg(feature = "wasm")]
pub fn host_get_status_code() -> u32 {
    unsafe { get_status_code() }
}

/// Helper function to read data from the host into a buffer.
///
/// # Arguments
///
/// * `read_fn` - A function that reads data into a buffer and returns the number of bytes read.
///
/// # Returns
///
/// Returns the read data as a string, or an error if reading fails.
#[cfg(feature = "wasm")]
fn read_from_buffer<F: Fn(*mut u8, i32) -> i32>(read_fn: F) -> Result<String> {
    let mut buffer = vec![0u8; 4096];
    let len = read_fn(buffer.as_mut_ptr(), buffer.len() as i32);

    if len < 0 {
        Err(TreblleError::HostFunction("Failed to read from buffer".to_string()))
    } else {
        buffer.truncate(len as usize);
        String::from_utf8(buffer).map_err(|e| TreblleError::HostFunction(e.to_string()))
    }
}
