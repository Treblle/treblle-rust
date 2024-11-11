//! Host functions module for the Treblle middleware.
//!
//! This module provides the interface between the WebAssembly module and the host environment (Traefik).
//! All functions are divided into categories based on HTTP Handler ABI specification:
//! * Administrative functions (config, features)
//! * Logging functions
//! * Header functions
//! * Body functions
//! * Request-specific functions
//! * Response-specific functions

#[cfg(not(test))]
use core::str;
#[cfg(not(test))]
use std::ffi::CString;
use treblle_core::{Result, TreblleError};

#[cfg(not(test))]
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

/// Default buffer size for reading from host
const DEFAULT_BUFFER_SIZE: usize = 4096;

/// Logs a message to the host environment.
///
/// # Arguments
///
/// * `level` - The log level (use constants from `crate::constants::log_level`).
/// * `message` - The message to log.
pub fn host_log(level: i32, message: &str) {
    #[cfg(test)]
    {
        let mut state = test::TEST_STATE.lock().unwrap();
        state.log_messages.push((level, message.to_string()));
    }

    #[cfg(not(test))]
    {
        let sanitized_message = message.replace('\0', "");
        if let Ok(c_message) = CString::new(sanitized_message) {
            unsafe {
                log(level, c_message.as_ptr() as *const u8, c_message.as_bytes().len() as u32);
            }
        }
    }
}

/// Gets configuration from the host environment.
///
/// Configuration is provided as JSON but follows Treblle's YAML structure:
/// ```yaml
/// apiKey: string
/// projectId: string
/// maskedFields: [string]
/// maskedFieldsRegex: [string]
/// ignoredRoutes: [string]
/// ignoredRoutesRegex: string
/// bufferResponse: bool
/// logLevel: string
/// rootCaPath: string
/// ```
pub fn host_get_config() -> Result<String> {
    #[cfg(test)]
    {
        Ok(test::TEST_STATE.lock().unwrap().config.clone())
    }

    #[cfg(not(test))]
    {
        read_from_buffer(|buf, buf_limit| unsafe { get_config(buf, buf_limit) })
    }
}

/// Enables features in the host environment.
///
/// # Arguments
///
/// * `features` - A bitfield of features to enable:
///   * 1: Buffer request body
///   * 2: Buffer response body
///   * 4: Enable trailers support
///
/// # Returns
///
/// Returns a bitfield of successfully enabled features.
pub fn host_enable_features(features: u32) -> u32 {
    #[cfg(test)]
    {
        test::TEST_STATE.lock().unwrap().enabled_features = features;
        features // In test mode, all requested features are enabled
    }

    #[cfg(not(test))]
    {
        unsafe { enable_features(features) }
    }
}

/// Gets HTTP method of the current request.
pub fn host_get_method() -> Result<String> {
    #[cfg(test)]
    {
        Ok(test::TEST_STATE.lock().unwrap().method.clone())
    }

    #[cfg(not(test))]
    {
        read_from_buffer(|buf, buf_limit| unsafe { get_method(buf, buf_limit) })
    }
}

/// Gets URI of the current request.
pub fn host_get_uri() -> Result<String> {
    #[cfg(test)]
    {
        Ok(test::TEST_STATE.lock().unwrap().uri.clone())
    }

    #[cfg(not(test))]
    {
        read_from_buffer(|buf, buf_limit| unsafe { get_uri(buf, buf_limit as u32) as i32 })
    }
}

/// Gets protocol version of the current request.
pub fn host_get_protocol_version() -> Result<String> {
    #[cfg(test)]
    {
        Ok(test::TEST_STATE.lock().unwrap().protocol_version.clone())
    }

    #[cfg(not(test))]
    {
        read_from_buffer(|buf, buf_limit| unsafe { get_protocol_version(buf, buf_limit) })
    }
}

/// Gets names of headers for a specific kind.
///
/// # Arguments
///
/// * `header_kind` - 0 for request, 1 for response, 2 for request trailers, 3 for response trailers
pub fn host_get_header_names(header_kind: u32) -> Result<String> {
    #[cfg(test)]
    {
        let state = test::TEST_STATE.lock().unwrap();
        Ok(state.headers.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>().join(","))
    }

    #[cfg(not(test))]
    {
        read_from_buffer(|buf, buf_limit| unsafe {
            get_header_names(header_kind, buf, buf_limit) as i32
        })
    }
}

/// Gets values for a specific header.
pub fn host_get_header_values(header_kind: u32, name: &str) -> Result<String> {
    #[cfg(test)]
    {
        let state = test::TEST_STATE.lock().unwrap();
        Ok(state
            .headers
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
            .join(","))
    }

    #[cfg(not(test))]
    {
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
}

/// Reads body data from request or response.
///
/// # Arguments
///
/// * `body_kind` - 0 for request, 1 for response
///
/// # Returns
///
/// Returns body data as bytes, or an error if reading fails.
pub fn host_read_body(body_kind: u32) -> Result<Vec<u8>> {
    #[cfg(test)]
    {
        Ok(test::TEST_STATE.lock().unwrap().body.clone())
    }

    #[cfg(not(test))]
    {
        let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
        let read = unsafe { read_body(body_kind, buffer.as_mut_ptr(), DEFAULT_BUFFER_SIZE as u32) };

        if read < 0 {
            return Err(TreblleError::HostFunction("Error reading body".to_string()));
        }

        let len = (read & 0xFFFFFFFF) as usize; // Extract length from lower 32 bits
        buffer.truncate(len);
        Ok(buffer)
    }
}

/// Writes body data back to request or response.
///
/// # Arguments
///
/// * `body_kind` - 0 for request, 1 for response
/// * `body` - Body data to write
///
/// # Returns
///
/// Returns Ok(()) on success, or error if writing fails.
pub fn host_write_body(body_kind: u32, body: &[u8]) -> Result<()> {
    #[cfg(test)]
    {
        let mut state = test::TEST_STATE.lock().unwrap();
        state.body = body.to_vec();
        Ok(())
    }

    #[cfg(not(test))]
    {
        unsafe {
            write_body(body_kind, body.as_ptr(), body.len() as u32);
        }
        Ok(())
    }
}

/// Gets the HTTP status code of the response.
///
/// # Returns
///
/// Returns status code as u32 (e.g., 200, 404, etc.)
pub fn host_get_status_code() -> u32 {
    #[cfg(test)]
    {
        test::TEST_STATE.lock().unwrap().status_code
    }

    #[cfg(not(test))]
    {
        unsafe { get_status_code() }
    }
}

/// Helper function to read data from the host into a buffer.
///
/// This function handles the common pattern of:
/// 1. Allocating a buffer
/// 2. Calling host function to fill buffer
/// 3. Converting result to string
/// 4. Handling errors
///
/// # Arguments
///
/// * `read_fn` - Function that reads data into buffer and returns length
///
/// # Returns
///
/// Returns string data read from host, or error if reading fails.
fn read_from_buffer<F>(read_fn: F) -> Result<String>
where
    F: Fn(*mut u8, i32) -> i32,
{
    let mut buffer = vec![0u8; DEFAULT_BUFFER_SIZE];
    let len = read_fn(buffer.as_mut_ptr(), buffer.len() as i32);

    if len < 0 {
        return Err(TreblleError::HostFunction("Failed to read from buffer".to_string()));
    }

    buffer.truncate(len as usize);
    String::from_utf8(buffer)
        .map_err(|e| TreblleError::HostFunction(format!("Invalid UTF-8 in response: {e}")))
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::test_utils;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct TestState {
        pub method: String,
        pub uri: String,
        pub protocol_version: String,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
        pub status_code: u32,
        pub config: String,
        pub log_messages: Vec<(i32, String)>,
        pub enabled_features: u32,
    }

    pub static TEST_STATE: Lazy<Mutex<TestState>> = Lazy::new(|| Mutex::new(TestState::default()));

    impl TestState {
        pub fn reset() {
            *TEST_STATE.lock().unwrap() = TestState::default();
        }
    }

    /// Sets up request state for testing
    pub fn setup_request(
        method: &str,
        uri: &str,
        protocol: &str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) {
        let mut state = TEST_STATE.lock().unwrap();
        state.method = method.to_string();
        state.uri = uri.to_string();
        state.protocol_version = protocol.to_string();
        state.headers = headers;
        state.body = body;
    }

    /// Sets up response state for testing
    pub fn setup_response(status_code: u32, headers: Vec<(String, String)>, body: Vec<u8>) {
        let mut state = TEST_STATE.lock().unwrap();
        state.status_code = status_code;
        state.headers = headers;
        state.body = body;
    }

    /// Sets up configuration for testing
    pub fn setup_config(config: &str) {
        let mut state = TEST_STATE.lock().unwrap();
        state.config = config.to_string();
    }

    #[test]
    fn test_host_get_method() {
        setup_request(
            "POST",
            "/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            vec![],
        );

        assert_eq!(host_get_method().unwrap(), "POST");
    }

    #[test]
    fn test_host_get_uri() {
        setup_request("GET", "/api/test?param=value", "HTTP/1.1", vec![], vec![]);

        assert_eq!(host_get_uri().unwrap(), "/api/test?param=value");
    }

    #[test]
    fn test_host_read_body() {
        let test_body = b"{\"test\":\"value\"}";
        setup_request("POST", "/test", "HTTP/1.1", vec![], test_body.to_vec());

        assert_eq!(host_read_body(0).unwrap(), test_body);
    }

    #[test]
    fn test_host_get_header_values() {
        setup_request(
            "GET",
            "/test",
            "HTTP/1.1",
            vec![
                ("accept".to_string(), "application/json".to_string()),
                ("accept".to_string(), "text/plain".to_string()),
            ],
            vec![],
        );

        assert_eq!(host_get_header_values(0, "accept").unwrap(), "application/json,text/plain");
    }

    #[test]
    fn test_status_code() {
        setup_response(404, vec![], vec![]);
        assert_eq!(host_get_status_code(), 404);
    }

    #[test]
    fn test_config_parsing() {
        let config = r#"{
            "apiKey": "test-key",
            "projectId": "test-project",
            "maskedFields": ["password", "token"],
            "maskedFieldsRegex": ["secret.*"],
            "ignoredRoutes": ["/health"],
            "ignoredRoutesRegex": ".*internal.*",
            "bufferResponse": true
        }"#;

        setup_config(config);
        assert_eq!(host_get_config().unwrap(), config);
    }

    #[test]
    fn test_body_operations() {
        let test_body = b"{\"key\":\"value\"}";

        // Test write
        host_write_body(0, test_body).unwrap();

        // Test read
        let read_body = host_read_body(0).unwrap();
        assert_eq!(read_body, test_body);
    }

    #[test]
    fn test_logging() {
        host_log(0, "Test message");
        let state = TEST_STATE.lock().unwrap();
        assert_eq!(state.log_messages.len(), 1);
        assert_eq!(state.log_messages[0], (0, "Test message".to_string()));
    }

    #[test]
    fn test_feature_enabling() {
        let features = 2; // Buffer response
        assert_eq!(host_enable_features(features), features);

        let state = TEST_STATE.lock().unwrap();
        assert_eq!(state.enabled_features, features);
    }

    #[test]
    fn test_invalid_header_name() {
        test_utils::setup_test_config();
        let result = host_get_header_values(0, "invalid\0header");
        assert!(result.is_err());
    }
}
