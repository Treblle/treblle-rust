use actix_web::dev::{ServiceRequest, ServiceResponse};
use std::time::Duration;
use treblle_core::payload::HttpExtractor;
use treblle_core::schema::{RequestInfo, ResponseInfo};

pub struct ActixExtractor;

impl HttpExtractor for ActixExtractor {
    type Request = ServiceRequest;
    type Response = ServiceResponse;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
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
            body: req.request().body().as_ref()
                .map(|b| String::from_utf8_lossy(b.as_ref()).to_string()),
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: res.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: res.response().body().size() as u64,
            load_time: duration.as_secs_f64(),
            body: match res.response().body() {
                actix_web::body::Body::Bytes(b) => Some(String::from_utf8_lossy(b).to_string()),
                _ => None,
            },
        }
    }
}