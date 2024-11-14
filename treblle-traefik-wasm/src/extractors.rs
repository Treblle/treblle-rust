use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

use crate::{
    constants::http::{REQUEST_KIND, RESPONSE_KIND},
    host_functions::{
        body::{host_read_body, host_write_body},
        headers::{host_get_header_names, host_get_header_values},
        request::{host_get_method, host_get_protocol_version, host_get_uri},
        response::host_get_status_code,
    },
    logger::{log, LogLevel},
    CONFIG,
};

use treblle_core::{
    constants::MAX_BODY_SIZE, utils::extract_ip_from_headers, ErrorInfo, RequestInfo, ResponseInfo,
    ServerInfo,
};

/// WASM data extractor for Treblle middleware
pub struct WasmExtractor;

/// Type aliases for empty request/response types
pub type Request = ();
pub type Response = ();

/// Clean up JSON Value to avoid String() wrapping
fn clean_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k, clean_json_value(v));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(clean_json_value).collect()),
        Value::String(s) => Value::String(s),
        _ => value,
    }
}

impl WasmExtractor {
    /// Extract and process request/response body
    fn extract_body(kind: u32) -> Option<Value> {
        log(LogLevel::Debug, &format!("Starting body extraction for kind: {kind}"));

        match host_read_body(kind) {
            Ok(body) => {
                // Add debug logging for body size
                log(LogLevel::Debug, &format!("Read body of size: {}", body.len()));

                if body.is_empty() {
                    log(LogLevel::Debug, "Body is empty, skipping");
                    return None;
                }

                if body.len() > MAX_BODY_SIZE {
                    log(
                        LogLevel::Debug,
                        &format!("Body size {} exceeds maximum {}", body.len(), MAX_BODY_SIZE),
                    );
                    return None;
                }

                // Write body back for next middleware if buffering is disabled
                if !CONFIG.buffer_request {
                    log(LogLevel::Debug, "Response buffering disabled, writing body back");
                    if let Err(e) = host_write_body(kind, &body) {
                        log(LogLevel::Error, &format!("Failed to write back body: {e}"));
                    }
                }

                // Parse JSON and ensure proper handling of string values
                match serde_json::from_slice(&body) {
                    Ok(json) => {
                        log(LogLevel::Debug, &format!("Successfully parsed JSON body: {:?}", json));
                        Some(clean_json_value(json))
                    }
                    Err(e) => {
                        log(
                            LogLevel::Warn,
                            &format!(
                                "Failed to parse JSON body: {e}, raw body: {:?}",
                                String::from_utf8_lossy(&body)
                            ),
                        );
                        None
                    }
                }
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Failed to read body: {e}"));
                None
            }
        }
    }

    /// Extract headers from WASM host
    fn extract_headers(kind: u32) -> HashMap<String, String> {
        log(LogLevel::Debug, &format!("Starting header extraction for kind: {kind}"));

        match host_get_header_names(kind) {
            Ok(header_names) => {
                let mut headers = HashMap::new();

                // Split on null bytes and commas, clean up empty strings
                let names: Vec<_> =
                    header_names.split(['\0', ',']).filter(|s| !s.is_empty()).collect();

                log(LogLevel::Debug, &format!("Found headers: {:?}", names));

                for name in names {
                    let name = name.trim();
                    if !name.is_empty() {
                        match host_get_header_values(kind, name) {
                            Ok(values) => {
                                log(LogLevel::Debug, &format!("Header '{name}' = '{values}'"));
                                headers.insert(name.to_string(), values);
                            }
                            Err(e) => log(
                                LogLevel::Error,
                                &format!("Failed to get values for header '{name}': {e}"),
                            ),
                        }
                    }
                }
                headers
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Failed to get header names: {e}"));
                HashMap::new()
            }
        }
    }
}

impl treblle_core::extractors::TreblleExtractor for WasmExtractor {
    type Request = Request;
    type Response = Response;

    fn extract_request_info(_req: &Self::Request) -> RequestInfo {
        log(LogLevel::Debug, "Starting request info extraction");

        let method = host_get_method().unwrap_or_else(|e| {
            log(LogLevel::Error, &format!("Failed to get method: {e}"));
            String::new()
        });

        let url = host_get_uri().unwrap_or_else(|e| {
            log(LogLevel::Error, &format!("Failed to get URI: {e}"));
            String::new()
        });

        let headers = Self::extract_headers(REQUEST_KIND);
        let user_agent = headers.get("user-agent").cloned().unwrap_or_default();

        // Create HeaderMap for IP extraction
        let mut header_map = http::HeaderMap::new();
        for (name, value) in &headers {
            if let (Ok(header_name), Ok(header_value)) = (
                http::header::HeaderName::from_bytes(name.as_bytes()),
                http::header::HeaderValue::from_str(value),
            ) {
                header_map.insert(header_name, header_value);
            }
        }

        let ip = extract_ip_from_headers(&header_map).unwrap_or_else(|| "unknown".to_string());

        let info = RequestInfo {
            timestamp: Utc::now(),
            ip,
            url,
            user_agent,
            method,
            headers,
            body: Self::extract_body(REQUEST_KIND),
        };

        log(LogLevel::Debug, &format!("Completed request info extraction: {:?}", info));

        info
    }

    fn extract_response_info(_res: &Self::Response, duration: Duration) -> ResponseInfo {
        log(LogLevel::Debug, "Starting response info extraction");

        let body = Self::extract_body(RESPONSE_KIND);
        log(LogLevel::Debug, &format!("Extracted response body: {:?}", body));

        let size = body.as_ref().map(|b| b.to_string().len() as u64).unwrap_or(0);

        let info = ResponseInfo {
            headers: Self::extract_headers(RESPONSE_KIND),
            code: u16::try_from(host_get_status_code()).expect("Failed to extract status code"),
            size,
            load_time: duration.as_secs_f64(),
            body,
        };

        log(LogLevel::Debug, &format!("Completed response info extraction: {:?}", info));

        info
    }

    fn extract_error_info(_res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        let status_code = host_get_status_code();

        if status_code >= 400 {
            let body = Self::extract_body(RESPONSE_KIND);

            // Try to extract error message from body if available
            let message = body
                .and_then(|value| match value {
                    Value::Object(map) => {
                        map.get("message").or_else(|| map.get("error")).map(ToString::to_string)
                    }
                    _ => Some(value.to_string()),
                })
                .unwrap_or_else(|| format!("HTTP error {status_code}"));

            Some(vec![ErrorInfo {
                source: "wasm".to_string(),
                error_type: format!("HTTP_{status_code}"),
                message,
                file: String::new(),
                line: 0,
            }])
        } else {
            None
        }
    }

    fn extract_server_info() -> ServerInfo {
        ServerInfo {
            ip: "unknown".to_string(),
            timezone: Utc::now().format("%Z").to_string(),
            software: None,
            signature: None,
            protocol: host_get_protocol_version().unwrap_or_else(|_| "HTTP/1.1".to_string()),
            encoding: None,
            os: treblle_core::schema::OsInfo {
                name: std::env::consts::OS.to_string(),
                release: "unknown".to_string(),
                architecture: std::env::consts::ARCH.to_string(),
            },
        }
    }
}
