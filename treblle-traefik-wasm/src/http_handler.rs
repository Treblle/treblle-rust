use std::time::Instant;
use http::header;

use treblle_core::schema::{RequestInfo, ResponseInfo, PayloadData, TrebllePayload};
use treblle_core::payload::PayloadBuilder;

use crate::host_functions;
use crate::constants;

pub struct WasmExtractor;

impl WasmExtractor {
    pub fn extract_request_info() -> RequestInfo {
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
            body: None, // Implement body extraction if needed
        }
    }

    pub fn build_request_payload(
        api_key: String,
        project_id: String,
    ) -> TrebllePayload {
        let request_info = Self::extract_request_info();

        PayloadBuilder::new(api_key, project_id)
            .with_data(PayloadData {
                request: request_info,
                response: ResponseInfo::default(),
                server: Default::default(), // Implement server info extraction
                language: Default::default(), // Implement language info
                errors: Vec::new(),
            })
            .build()
    }

    pub fn extract_response_info(start_time: Instant) -> ResponseInfo {
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
            size: 0, // Implement size extraction if possible
            load_time: start_time.elapsed().as_secs_f64(),
            body: None, // Implement body extraction if needed and if buffer_response is enabled
        }
    }

    pub fn build_response_payload(
        api_key: String,
        project_id: String,
        start_time: Instant,
    ) -> TrebllePayload {
        let response_info = Self::extract_response_info(start_time);

        PayloadBuilder::new(api_key, project_id)
            .with_data(PayloadData {
                request: RequestInfo::default(),
                response: response_info,
                server: Default::default(), // Implement server info extraction
                language: Default::default(), // Implement language info
                errors: Vec::new(),
            })
            .build()
    }
}