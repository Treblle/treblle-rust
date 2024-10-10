use std::time::Duration;
use http::header;
use treblle_core::schema::{RequestInfo, ResponseInfo};
use treblle_core::payload::HttpExtractor;
use crate::host_functions;
use crate::constants;
use crate::config::WasmConfig;

pub struct WasmExtractor;

impl HttpExtractor for WasmExtractor {
    type Request = ();
    type Response = ();

    fn extract_request_info(_req: &Self::Request) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: host_functions::host_get_header_values(constants::http::REQUEST_KIND, "X-Forwarded-For")
                .unwrap_or_else(|_| "unknown".to_string()),
            url: host_functions::host_get_uri().unwrap_or_default(),
            user_agent: host_functions::host_get_header_values(constants::http::REQUEST_KIND, header::USER_AGENT.as_str())
                .unwrap_or_default(),
            method: host_functions::host_get_method().unwrap_or_default(),
            headers: host_functions::host_get_header_names(constants::http::REQUEST_KIND)
                .unwrap_or_default()
                .split(',')
                .filter_map(|name| {
                    host_functions::host_get_header_values(constants::http::REQUEST_KIND, name)
                        .ok()
                        .map(|value| (name.to_string(), value))
                })
                .collect(),
            body: host_functions::host_read_body(constants::http::REQUEST_KIND).ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        }
    }

    fn extract_response_info(_res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: host_functions::host_get_header_names(constants::http::RESPONSE_KIND)
                .unwrap_or_default()
                .split(',')
                .filter_map(|name| {
                    host_functions::host_get_header_values(constants::http::RESPONSE_KIND, name)
                        .ok()
                        .map(|value| (name.to_string(), value))
                })
                .collect(),
            code: host_functions::host_get_status_code(),
            size: host_functions::host_get_response_size().unwrap_or(0),
            load_time: duration.as_secs_f64(),
            body: if CONFIG.buffer_response {
                host_functions::host_read_body(constants::http::RESPONSE_KIND).ok()
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            } else {
                None
            },
        }
    }
}