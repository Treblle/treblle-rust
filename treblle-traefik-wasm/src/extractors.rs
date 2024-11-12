use std::collections::HashMap;
use std::time::Duration;

use http::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use treblle_core::{
    constants::MAX_BODY_SIZE,
    utils::extract_ip_from_headers,
    ErrorInfo, RequestInfo, ResponseInfo, ServerInfo, TreblleError,
};
use chrono::Utc;

use crate::{
    constants::http::{REQUEST_KIND, RESPONSE_KIND},
    host_functions::{
        host_get_header_names, host_get_header_values, host_get_method, host_get_protocol_version,
        host_get_status_code, host_get_uri, host_read_body,
    },
    logger::{log, LogLevel},
};

/// Empty struct to implement TreblleExtractor trait for Traefik WASM middleware
pub struct WasmExtractor;

// Type aliases for cleaner code
pub type Request = (); // Traefik doesn't provide a request type in WASM
pub type Response = (); // Traefik doesn't provide a response type in WASM

// Store request and response bodies globally since we can't use extensions in WASM
static REQUEST_BODY: once_cell::sync::OnceCell<Vec<u8>> = once_cell::sync::OnceCell::new();
static RESPONSE_BODY: once_cell::sync::OnceCell<Vec<u8>> = once_cell::sync::OnceCell::new();

impl WasmExtractor {
    /// Store the request body for later use
    pub fn store_request_body(body: Vec<u8>) {
        if let Err(_) = REQUEST_BODY.set(body) {
            log(LogLevel::Error, "Failed to store request body");
        }
    }

    /// Store the response body for later use
    pub fn store_response_body(body: Vec<u8>) {
        if RESPONSE_BODY.set(body).is_err() {
            log(LogLevel::Error, "Failed to store response body");
        }
    }

    /// Get stored request body
    pub fn get_request_body() -> Option<&'static [u8]> {
        REQUEST_BODY.get().map(Vec::as_slice)
    }

    /// Get stored response body
    pub fn get_response_body() -> Option<&'static [u8]> {
        RESPONSE_BODY.get().map(Vec::as_slice)
    }

    /// Reads and processes the request body
    pub fn read_request_body() -> Result<Value, TreblleError> {
        if let Some(body) = Self::get_request_body() {
            Self::parse_body(body)
        } else {
            let body = Self::read_raw_body(REQUEST_KIND)?;
            Self::store_request_body(body.clone());
            Self::parse_body(&body)
        }
    }

    /// Reads and processes the response body
    pub fn read_response_body() -> Result<Value, TreblleError> {
        if let Some(body) = Self::get_response_body() {
            Self::parse_body(body)
        } else {
            let body = Self::read_raw_body(RESPONSE_KIND)?;
            Self::store_response_body(body.clone());
            Self::parse_body(&body)
        }
    }

    /// Helper method to read raw body data
    fn read_raw_body(kind: u32) -> Result<Vec<u8>, TreblleError> {
        let headers = Self::extract_headers(kind)?;
        let http_headers = Self::convert_headers(&headers);

        if !Self::is_json_content(&http_headers) {
            log(LogLevel::Debug, "Non-JSON content type, skipping body processing");
            return Ok(Vec::new());
        }

        // Read the raw body
        let body = host_read_body(kind)?;

        // Check body size
        if body.len() > MAX_BODY_SIZE {
            log(LogLevel::Warn, "Body size exceeds maximum allowed size");
            return Ok(Vec::new());
        }

        Ok(body)
    }

    /// Helper method to parse body data as JSON
    fn parse_body(body: &[u8]) -> Result<Value, TreblleError> {
        if body.is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_slice(body).map_err(|e| {
            log(LogLevel::Warn, &format!("Failed to parse JSON body: {e}"));
            TreblleError::Json(e)
        })
    }

    /// Helper method to check if content type is JSON
    fn is_json_content(headers: &HeaderMap) -> bool {
        headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .map(|ct| ct.to_lowercase().contains("application/json"))
            .unwrap_or(false)
    }

    /// Convert HashMap headers to http::HeaderMap
    fn convert_headers(headers: &HashMap<String, String>) -> HeaderMap {
        let mut http_headers = HeaderMap::new();
        for (key, value) in headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                http_headers.insert(name, val);
            } else {
                log(LogLevel::Warn, &format!("Failed to convert header: {key}:{value}"));
            }
        }
        http_headers
    }

    /// Enhanced helper to extract headers with better error handling
    fn extract_headers(kind: u32) -> Result<HashMap<String, String>, TreblleError> {
        log(LogLevel::Debug, "Extracting headers");
        let header_names = host_get_header_names(kind)?;
        let mut headers = HashMap::new();

        for name in header_names.split(',').filter(|s| !s.is_empty()) {
            match host_get_header_values(kind, name) {
                Ok(values) => {
                    headers.insert(name.to_string(), values);
                }
                Err(e) => {
                    log(LogLevel::Warn, &format!("Failed to get values for header {name}: {e}"));
                }
            }
        }

        log(LogLevel::Debug, &format!("Extracted {} headers", headers.len()));
        Ok(headers)
    }
}

impl treblle_core::extractors::TreblleExtractor for WasmExtractor {
    type Request = Request;
    type Response = Response;

    fn extract_request_info(_req: &Self::Request) -> RequestInfo {
        log(LogLevel::Debug, "Extracting request information");

        let method = host_get_method()
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to get method: {e}"));
            })
            .unwrap_or_default();

        let url = host_get_uri()
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to get URI: {e}"));
            })
            .unwrap_or_default();

        let headers = Self::extract_headers(REQUEST_KIND)
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to get request headers: {e}"));
            })
            .unwrap_or_default();

        let http_headers = Self::convert_headers(&headers);
        let user_agent = http_headers
            .get(http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let ip = extract_ip_from_headers(&http_headers).unwrap_or_else(|| "unknown".to_string());

        let body = Self::read_request_body()
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to read request body: {e}"));
                Value::Null
            })
            .ok();

        log(LogLevel::Debug, "Request information extracted successfully");

        RequestInfo {
            timestamp: Utc::now(),
            ip,
            url,
            user_agent,
            method,
            headers,
            body,
        }
    }

    fn extract_response_info(_res: &Self::Response, duration: Duration) -> ResponseInfo {
        log(LogLevel::Debug, "Extracting response information");

        let status_code = host_get_status_code();

        let headers = Self::extract_headers(RESPONSE_KIND)
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to get response headers: {e}"));
            })
            .unwrap_or_default();

        let body = Self::read_response_body()
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to read response body: {e}"));
                Value::Null
            })
            .ok();

        let size = body.as_ref().map(|b| b.to_string().len()).unwrap_or(0) as u64;

        log(LogLevel::Debug, "Response information extracted successfully");

        ResponseInfo {
            headers,
            code: u16::try_from(status_code).expect("Failed extracting status code"),
            size,
            load_time: duration.as_secs_f64(),
            body,
        }
    }

    fn extract_error_info(_res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        let status_code = host_get_status_code();

        if status_code >= 400 {
            log(LogLevel::Debug, &format!("Extracting error info for status code {status_code}"));
            Some(vec![ErrorInfo {
                source: "http".to_string(),
                error_type: format!("HTTP_{status_code}"),
                message: format!("HTTP error {status_code}"),
                file: String::new(),
                line: 0,
            }])
        } else {
            None
        }
    }

    fn extract_server_info() -> ServerInfo {
        log(LogLevel::Debug, "Extracting server information");

        let protocol = host_get_protocol_version()
            .map_err(|e| {
                log(LogLevel::Error, &format!("Failed to get protocol version: {e}"));
            })
            .unwrap_or_else(|_| "HTTP/1.1".to_string());

        ServerInfo {
            ip: "unknown".to_string(), // WASM environment cannot access host IP
            timezone: Utc::now().format("%Z").to_string(),
            software: None,
            signature: None,
            protocol,
            encoding: None,
            os: treblle_core::schema::OsInfo {
                name: std::env::consts::OS.to_string(),
                release: "unknown".to_string(),
                architecture: std::env::consts::ARCH.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_headers() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-forwarded-for".to_string(), "203.0.113.195".to_string());

        let http_headers = WasmExtractor::convert_headers(&headers);

        assert_eq!(http_headers.get("content-type").unwrap().to_str().unwrap(), "application/json");
        assert_eq!(
            http_headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
            "203.0.113.195"
        );
    }

    #[test]
    fn test_is_json_content() {
        let mut headers = HeaderMap::new();

        headers.insert(http::header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        assert!(WasmExtractor::is_json_content(&headers));

        headers.clear();
        headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        assert!(WasmExtractor::is_json_content(&headers));

        headers.clear();
        headers.insert(http::header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        assert!(!WasmExtractor::is_json_content(&headers));
    }

    #[test]
    fn test_body_storage() {
        let test_body = b"test data".to_vec();
        WasmExtractor::store_request_body(test_body.clone());
        assert_eq!(WasmExtractor::get_request_body(), Some(test_body.as_slice()));
    }
}