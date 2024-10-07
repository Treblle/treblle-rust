use actix_web::dev::{ServiceRequest, ServiceResponse};
use std::time::Instant;

use treblle_core::schema::{RequestInfo, ResponseInfo, PayloadData, TrebllePayload};
use treblle_core::payload::PayloadBuilder;

pub struct ActixExtractor;

impl ActixExtractor {
    pub fn extract_request_info(req: &ServiceRequest) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: req.connection_info().realip_remote_addr()
                .unwrap_or("unknown").to_string(),
            url: req.uri().to_string(),
            user_agent: req.headers().get("User-Agent")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("").to_string(),
            method: req.method().to_string(),
            headers: req.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            body: None, // You might want to implement body extraction if needed
        }
    }

    pub fn build_request_payload(
        api_key: String,
        project_id: String,
        req: &ServiceRequest,
    ) -> TrebllePayload {
        let request_info = Self::extract_request_info(req);

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

    pub fn extract_response_info(res: &ServiceResponse, start_time: Instant) -> ResponseInfo {
        ResponseInfo {
            headers: res.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: res.response().body().size() as u64,
            load_time: start_time.elapsed().as_secs_f64(),
            body: None, // You might want to implement body extraction if needed
        }
    }

    pub fn build_response_payload(
        api_key: String,
        project_id: String,
        res: &ServiceResponse,
        start_time: Instant,
    ) -> TrebllePayload {
        let response_info = Self::extract_response_info(res, start_time);

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