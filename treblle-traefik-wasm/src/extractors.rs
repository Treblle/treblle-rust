//! WASM extractor implementation for Treblle middleware.
//!
//! This module provides the implementation of the HttpExtractor trait for
//! extracting request/response information from WASM host functions.

use crate::{
    constants::http::{REQUEST_KIND, RESPONSE_KIND},
    host_functions, CONFIG,
};
use chrono::Utc;
use http::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use treblle_core::{
    payload::HttpExtractor,
    schema::{ErrorInfo, RequestInfo, ResponseInfo},
    utils::extract_ip_from_headers,
};

/// WASM extractor for Treblle middleware
pub struct WasmExtractor;

impl WasmExtractor {
    /// Convert HashMap headers to HeaderMap for compatibility with treblle-core
    fn convert_header_hash_map_to_header_map(headers: &HashMap<String, String>) -> HeaderMap {
        let mut header_map = HeaderMap::new();
        for (key, value) in headers {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                if let Ok(header_name) = http::header::HeaderName::from_bytes(key.as_bytes()) {
                    header_map.insert(header_name, header_value);
                }
            }
        }
        header_map
    }
}

impl HttpExtractor for WasmExtractor {
    type Request = ();
    type Response = ();

    fn extract_request_info(_req: &Self::Request) -> RequestInfo {
        // Collect request headers into a HashMap
        let headers: HashMap<String, String> = host_functions::host_get_header_names(REQUEST_KIND)
            .unwrap_or_default()
            .split(',')
            .filter_map(|name| {
                if name.is_empty() {
                    return None;
                }

                host_functions::host_get_header_values(REQUEST_KIND, name)
                    .ok()
                    .map(|value| (name.to_string(), value))
            })
            .collect();

        // Try to parse request body as JSON
        let body = host_functions::host_read_body(REQUEST_KIND)
            .ok()
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
            });

        // Extract IP from headers
        let ip = extract_ip_from_headers(&Self::convert_header_hash_map_to_header_map(&headers))
            .unwrap_or_else(|| "unknown".to_string());

        RequestInfo {
            timestamp: Utc::now(),
            ip,
            url: host_functions::host_get_uri().unwrap_or_default(),
            user_agent: headers.get("user-agent").cloned().unwrap_or_default(),
            method: host_functions::host_get_method().unwrap_or_default(),
            headers,
            body,
        }
    }

    fn extract_response_info(_res: &Self::Response, duration: Duration) -> ResponseInfo {
        // Collect response headers
        let headers: HashMap<String, String> = host_functions::host_get_header_names(RESPONSE_KIND)
            .unwrap_or_default()
            .split(',')
            .filter_map(|name| {
                if name.is_empty() {
                    return None;
                }

                host_functions::host_get_header_values(RESPONSE_KIND, name)
                    .ok()
                    .map(|value| (name.to_string(), value))
            })
            .collect();

        // Try to read response body if buffering is enabled
        let body = if CONFIG.buffer_response {
            host_functions::host_read_body(RESPONSE_KIND)
                .ok()
                .and_then(|bytes| {
                    String::from_utf8(bytes)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                })
        } else {
            None
        };

        // Get content length if available
        let size = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        ResponseInfo {
            headers,
            code: host_functions::host_get_status_code() as u16,
            size,
            load_time: duration.as_secs_f64(),
            body,
        }
    }

    fn extract_error_info(_res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        let status_code = host_functions::host_get_status_code();
        if status_code >= 400 {
            let body = host_functions::host_read_body(RESPONSE_KIND)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|s| serde_json::from_str::<Value>(&s).ok());

            let message = match body {
                Some(Value::Object(map)) => map
                    .get("message")
                    .or_else(|| map.get("error"))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| format!("HTTP {}", status_code)),
                Some(value) => value.to_string(),
                None => format!("HTTP {}", status_code),
            };

            Some(vec![ErrorInfo {
                source: "wasm".to_string(),
                error_type: format!("HTTP_{}", status_code),
                message,
                file: String::new(),
                line: 0,
            }])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_functions::test as host_test;
    use serde_json::json;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            let config_json = r#"{
                "apiKey": "test_key",
                "projectId": "test_project",
                "ignoredRoutes": ["/health"],
                "ignoredRoutesRegex": ["^/internal/.*$"],
                "bufferResponse": true
            }"#;
            host_test::setup_config(config_json);
        });
    }

    #[test]
    fn test_extract_request_info() {
        host_test::setup_request(
            "POST",
            "/api/test",
            "HTTP/1.1",
            vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-forwarded-for".to_string(), "127.0.0.1".to_string()),
                ("user-agent".to_string(), "test-agent".to_string()),
            ],
            json!({ "test": "value" }).to_string().into_bytes(),
        );

        let info = WasmExtractor::extract_request_info(&());

        assert_eq!(info.method, "POST");
        assert_eq!(info.url, "/api/test");
        assert_eq!(info.ip, "127.0.0.1");
        assert_eq!(info.user_agent, "test-agent");
        assert!(info.body.is_some());
    }

    #[test]
    fn test_extract_response_info() {
        setup();
        host_test::setup_response(
            200,
            vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("content-length".to_string(), "123".to_string()),
            ],
            json!({ "result": "success" }).to_string().into_bytes(),
        );

        let info = WasmExtractor::extract_response_info(&(), Duration::from_secs(1));

        assert_eq!(info.code, 200);
        assert_eq!(info.size, 123);
        assert_eq!(info.load_time, 1.0);
        if CONFIG.buffer_response {
            assert!(info.body.is_some());
        }
    }

    #[test]
    fn test_empty_body_handling() {
        host_test::setup_request("GET", "/test", "HTTP/1.1", vec![], vec![]);

        let info = WasmExtractor::extract_request_info(&());
        assert!(info.body.is_none());
    }

    #[test]
    fn test_invalid_json_body() {
        host_test::setup_request(
            "POST",
            "/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            "invalid json".as_bytes().to_vec(),
        );

        let info = WasmExtractor::extract_request_info(&());
        assert!(info.body.is_none());
    }

    #[test]
    fn test_ip_extraction() {
        // Test X-Forwarded-For
        host_test::setup_request(
            "GET",
            "/test",
            "HTTP/1.1",
            vec![("x-forwarded-for".to_string(), "192.168.1.1".to_string())],
            vec![],
        );
        let info = WasmExtractor::extract_request_info(&());
        assert_eq!(info.ip, "192.168.1.1");

        // Test X-Real-IP
        host_test::setup_request(
            "GET",
            "/test",
            "HTTP/1.1",
            vec![("x-real-ip".to_string(), "10.0.0.1".to_string())],
            vec![],
        );
        let info = WasmExtractor::extract_request_info(&());
        assert_eq!(info.ip, "10.0.0.1");

        // Test Forwarded header (RFC 7239)
        host_test::setup_request(
            "GET",
            "/test",
            "HTTP/1.1",
            vec![("forwarded".to_string(), "for=172.16.1.1".to_string())],
            vec![],
        );
        let info = WasmExtractor::extract_request_info(&());
        assert_eq!(info.ip, "172.16.1.1");
    }

    #[test]
    fn test_error_extraction() {
        host_test::setup_response(
            404,
            vec![("content-type".to_string(), "application/json".to_string())],
            json!({
                "error": "Not Found",
                "message": "Resource does not exist"
            })
            .to_string()
            .into_bytes(),
        );

        let errors = WasmExtractor::extract_error_info(&()).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, "HTTP_404");
        assert!(errors[0].message.contains("Resource does not exist"));
    }
}
