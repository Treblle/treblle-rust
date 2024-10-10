use rocket::Request;
use rocket::Response;
use std::time::Duration;
use treblle_core::payload::HttpExtractor;
use treblle_core::schema::{RequestInfo, ResponseInfo};

pub struct RocketExtractor;

impl HttpExtractor for RocketExtractor {
    type Request = Request<'_>;
    type Response = Response<'_>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: req.client_ip().map(|ip| ip.to_string()).unwrap_or_else(|| "unknown".to_string()),
            url: req.uri().to_string(),
            user_agent: req.headers().get_one("User-Agent").unwrap_or("").to_string(),
            method: req.method().to_string(),
            headers: req.headers().iter()
                .map(|header| (header.name().to_string(), header.value().to_string()))
                .collect(),
            body: req.body_string().ok(),
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: res.headers().iter()
                .map(|header| (header.name().to_string(), header.value().to_string()))
                .collect(),
            code: res.status().code as u16,
            size: res.body().size().map(|s| s as u64).unwrap_or(0),
            load_time: duration.as_secs_f64(),
            body: res.body_string().ok(),
        }
    }
}