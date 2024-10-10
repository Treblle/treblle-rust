use axum::http::{Request, Response};
use axum::body::Body;
use std::time::Duration;
use treblle_core::payload::HttpExtractor;
use treblle_core::schema::{RequestInfo, ResponseInfo};
use hyper::body::Bytes;

pub struct AxumExtractor;

impl HttpExtractor for AxumExtractor {
    type Request = Request<Body>;
    type Response = Response<Body>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: req.extensions().get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            url: req.uri().to_string(),
            user_agent: req.headers().get("User-Agent")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("").to_string(),
            method: req.method().to_string(),
            headers: req.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            body: req.extensions().get::<Bytes>()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string()),
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: res.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: res.extensions().get::<u64>().cloned().unwrap_or(0),
            load_time: duration.as_secs_f64(),
            body: res.extensions().get::<Bytes>()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string()),
        }
    }
}