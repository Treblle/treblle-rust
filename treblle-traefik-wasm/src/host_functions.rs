//! Host functions module for the Treblle middleware.
//!
//! This module provides the interface between the WebAssembly module
//! and the host environment (Traefik).

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

    fn get_header_names(header_kind: u32, buf: *mut u8, buf_limit: i32) -> i64;
    fn get_header_values(
        header_kind: u32,
        name_ptr: *const u8,
        name_len: u32,
        buf: *mut u8,
        buf_limit: i32,
    ) -> i64;

    // Body only methods
    fn read_body(body_kind: u32, ptr: *mut u8, buf_limit: u32) -> i64;
    fn write_body(body_kind: u32, ptr: *const u8, message_len: u32);

    // Request only methods
    fn get_method(buf: *mut u8, buf_limit: i32) -> i32;
    fn get_uri(buf: *mut u8, buf_limit: u32) -> i32;
    fn get_protocol_version(buf: *mut u8, buf_limit: i32) -> i32;
    fn get_source_addr(buf: *mut u8, buf_limit: i32) -> i32;

    // Response only methods
    fn get_status_code() -> u32;
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
pub fn host_enable_features(features: u32) -> Result<u32> {
    let enabled = unsafe { enable_features(features) };

    // Verify the requested features were enabled
    if enabled & features != features {
        return Err(TreblleError::HostFunction(format!(
            "Failed to enable features. Requested: {features}, Enabled: {enabled}"
        )));
    }

    Ok(enabled)
}

/// Retrieves the configuration from the host environment if it isn't larger than `buf_limit`.
///
/// # Returns
///
/// Returns the configuration as a string, or an error if retrieval fails.
#[cfg(feature = "wasm")]
pub fn host_get_config() -> Result<String> {
    read_from_buffer(|buf, buf_limit| unsafe { get_config(buf, buf_limit) })
}

pub mod log {
    use crate::host_functions::log;
    use std::ffi::CString;

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
}

pub mod headers {
    use crate::host_functions::{get_header_names, get_header_values, read_from_buffer};
    use std::ffi::CString;
    use treblle_core::{Result, TreblleError};

    /// Retrieves the header names for a given header kind if it isn't larger than `buf_limit`.
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

    /// Retrieves the values for a specific header if it isn't larger than `buf_limit`.
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
            .map_err(|e| TreblleError::HostFunction(format!("Invalid header name: {e}")))?;

        read_from_buffer(|buf, buf_limit| {
            let result = unsafe {
                get_header_values(
                    header_kind,
                    c_name.as_ptr() as *const u8,
                    c_name.as_bytes().len() as u32,
                    buf,
                    buf_limit,
                )
            };

            // Check for negative values indicating errors
            if result < 0 {
                return -1;
            }

            result as i32
        })
    }
}

pub mod body {
    use crate::host_functions::{read_body, write_body};
    use treblle_core::Result;

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
        let mut buffer = vec![0u8; 4096];
        let mut complete_body = Vec::new();

        loop {
            // read_body is stateful, so repeated calls read what's remaining in the stream, as opposed to starting from zero
            let result = unsafe { read_body(body_kind, buffer.as_mut_ptr(), buffer.len() as u32) };
            // The high 32-bits is EOF (1 or 0) and the low 32-bits is the length
            let eof = (result >> 32) & 1;
            let len = result as u32;

            if len > 0 {
                complete_body.extend_from_slice(&buffer[..len as usize]);
            }

            if eof == 1 {
                break;
            }
        }

        Ok(complete_body)
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
}

pub mod request {
    use crate::host_functions::{
        get_method, get_protocol_version, get_source_addr, get_uri, read_from_buffer,
    };
    use treblle_core::Result;

    /// Retrieves the HTTP method of the current request if it isn't larger than `buf_limit`.
    ///
    /// # Returns
    ///
    /// Returns the HTTP method as a string, e.g. `"GET"`, or an error if retrieval fails.
    #[cfg(feature = "wasm")]
    pub fn host_get_method() -> Result<String> {
        read_from_buffer(|buf, buf_limit| unsafe { get_method(buf, buf_limit) })
    }

    /// Retrieves the URI of the current request if it isn't larger than `buf_limit`.
    /// The result is its length in bytes.
    ///
    /// The host should return "/" instead of empty for a request with no URI.
    ///
    /// The URI may include query parameters. It will always write the URI encoded
    //  to ASCII (both path and query parameters e.g. "/v1.0/hi?name=kung+fu+panda"). See
    //  https://datatracker.ietf.org/doc/html/rfc3986#section-2 for more references.
    ///
    /// # Returns
    ///
    /// Returns the URI as a string, e.g. `"/v1.0/hi?name=panda"`, or an error if retrieval fails.
    #[cfg(feature = "wasm")]
    pub fn host_get_uri() -> Result<String> {
        read_from_buffer(|buf, buf_limit| unsafe { get_uri(buf, buf_limit as u32) })
    }

    /// Retrieves the address of the current request if it isn't larger than `buf_limit`.
    /// The result is its length in bytes. Ex. `"1.1.1.1:12345"` or `"[fe80::101e:2bdf:8bfb:b97e]:12345"`
    ///
    /// # Returns
    ///
    ///  Returns the source address as a string, or an error if retrieval fails.
    #[cfg(feature = "wasm")]
    pub fn host_get_source_addr() -> Result<String> {
        read_from_buffer(|buf, buf_limit| unsafe { get_source_addr(buf, buf_limit) })
    }

    /// Retrieves the protocol version of the current request if it isn't larger than `buf_limit`.
    /// The result is its length in bytes.
    /// The most common protocol versions are "HTTP/1.1" and "HTTP/2.0".
    ///
    /// # Returns
    ///
    /// Returns the protocol version as a string, or an error if retrieval fails.
    /// The result is its length in bytes.
    #[cfg(feature = "wasm")]
    pub fn host_get_protocol_version() -> Result<String> {
        read_from_buffer(|buf, buf_limit| unsafe { get_protocol_version(buf, buf_limit) })
    }
}

pub mod response {
    use crate::host_functions::get_status_code;

    /// Retrieves the status code of the current response.
    ///
    /// # Returns
    ///
    /// Returns the status code as an u32, e.g. 200.
    #[cfg(feature = "wasm")]
    pub fn host_get_status_code() -> u32 {
        unsafe { get_status_code() }
    }
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
