//! WASM extractor implementation for Treblle middleware.
//!
//! This module provides the implementation of the TreblleExtractor trait for
//! extracting request/response information from WASM host functions.

use crate::{
    constants::http::{REQUEST_KIND, RESPONSE_KIND},
    host_functions, CONFIG,
};
use chrono::Utc;
use http::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use treblle_core::{extractors::TreblleExtractor, schema::OsInfo};
use treblle_core::{
    schema::{ErrorInfo, RequestInfo, ResponseInfo},
    utils::extract_ip_from_headers,
    ServerInfo,
};

static SERVER_INFO: OnceLock<ServerInfo> = OnceLock::new();

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

impl TreblleExtractor for WasmExtractor {
    type Request = ();
    type Response = ();

    fn extract_request_info(_req: &Self::Request) -> RequestInfo {
        // Collect request headers into a HashMap
        let headers: HashMap<String, String> =
            host_functions::host_get_header_names(crate::constants::http::REQUEST_KIND)
                .unwrap_or_default()
                .split(',')
                .filter_map(|name| {
                    if name.is_empty() {
                        return None;
                    }
                    host_functions::host_get_header_values(
                        crate::constants::http::REQUEST_KIND,
                        name,
                    )
                    .ok()
                    .map(|value| (name.to_string(), value))
                })
                .collect();

        // Convert headers for IP extraction
        let header_map = Self::convert_header_hash_map_to_header_map(&headers);

        // Try to parse request body as JSON
        let body = host_functions::host_read_body(crate::constants::http::REQUEST_KIND)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|s| serde_json::from_str(&s).ok());

        RequestInfo {
            timestamp: Utc::now(),
            ip: extract_ip_from_headers(&header_map).unwrap_or_else(|| "unknown".to_string()),
            url: host_functions::host_get_uri().unwrap_or_default(),
            user_agent: headers.get("user-agent").cloned().unwrap_or_default(),
            method: host_functions::host_get_method().unwrap_or_default(),
            headers,
            body,
        }
    }

    fn extract_response_info(_res: &Self::Response, duration: Duration) -> ResponseInfo {
        let headers: HashMap<String, String> =
            host_functions::host_get_header_names(crate::constants::http::RESPONSE_KIND)
                .unwrap_or_default()
                .split(',')
                .filter_map(|name| {
                    if name.is_empty() {
                        return None;
                    }
                    host_functions::host_get_header_values(
                        crate::constants::http::RESPONSE_KIND,
                        name,
                    )
                    .ok()
                    .map(|value| (name.to_string(), value))
                })
                .collect();

        let body = if CONFIG.buffer_response {
            host_functions::host_read_body(crate::constants::http::RESPONSE_KIND)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        };

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

    fn extract_server_info() -> ServerInfo {
        SERVER_INFO
            .get_or_init(|| {
                ServerInfo {
                    ip: "wasm".to_string(),      // Since we're in WASM, we can't get local IP
                    timezone: "UTC".to_string(), // WASM environment defaults to UTC
                    software: Some(format!("traefik-wasm/{}", env!("CARGO_PKG_VERSION"))),
                    signature: None,
                    protocol: "HTTP/1.1".to_string(),
                    encoding: None,
                    os: OsInfo {
                        name: "wasm".to_string(),
                        release: "1.0".to_string(),
                        architecture: "wasm32".to_string(),
                    },
                }
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{host_functions::test as host_test, test_utils};
    use serde_json::json;

    fn setup() {
        test_utils::setup_test_config();
    }

    #[test]
    fn test_extract_request_info() {
        setup();
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
        if let Some(body) = info.body {
            assert_eq!(body["test"], "value");
        }
    }

    #[test]
    fn test_empty_body_handling() {
        setup();
        host_test::setup_request("GET", "/test", "HTTP/1.1", vec![], vec![]);

        let info = WasmExtractor::extract_request_info(&());
        assert!(info.body.is_none());
    }

    #[test]
    fn test_invalid_json_body() {
        setup();
        host_test::setup_request(
            "POST",
            "/test",
            "HTTP/1.1",
            vec![("content-type".to_string(), "application/json".to_string())],
            b"invalid json".to_vec(),
        );

        let info = WasmExtractor::extract_request_info(&());
        assert!(info.body.is_none());
    }

    #[test]
    fn test_extract_response_info() {
        setup();
        host_test::setup_response(
            200,
            vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("content-length".to_string(), "23".to_string()),
            ],
            json!({ "result": "success" }).to_string().into_bytes(),
        );

        let info = WasmExtractor::extract_response_info(&(), Duration::from_secs(1));

        assert_eq!(info.code, 200);
        assert_eq!(info.size, 23);
        assert_eq!(info.load_time, 1.0);
        if let Some(body) = info.body {
            assert_eq!(body["result"], "success");
        }
    }

    #[test]
    fn test_ip_extraction() {
        setup();
        let test_cases = vec![
            // Test X-Forwarded-For
            (
                vec![("x-forwarded-for".to_string(), "192.168.1.1".to_string())],
                "192.168.1.1",
            ),
            // Test X-Real-IP
            (
                vec![("x-real-ip".to_string(), "10.0.0.1".to_string())],
                "10.0.0.1",
            ),
            // Test Forwarded header
            (
                vec![("forwarded".to_string(), "for=172.16.1.1".to_string())],
                "172.16.1.1",
            ),
        ];

        for (headers, expected_ip) in test_cases {
            host_test::setup_request("GET", "/test", "HTTP/1.1", headers, vec![]);
            let info = WasmExtractor::extract_request_info(&());
            assert_eq!(info.ip, expected_ip);
        }
    }

    #[test]
    fn test_error_extraction() {
        setup();
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
