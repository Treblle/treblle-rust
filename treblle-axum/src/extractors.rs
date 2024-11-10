use axum::body::Body;
use axum::http::{Request, Response};
use hyper::body::Bytes;
use serde_json::Value;
use std::time::Duration;
use treblle_core::payload::HttpExtractor;
use treblle_core::schema::{ErrorInfo, RequestInfo, ResponseInfo};

pub struct AxumExtractor;

impl HttpExtractor for AxumExtractor {
    type Request = Request<Body>;
    type Response = Response<Body>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: req
                .extensions()
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            url: req.uri().to_string(),
            user_agent: req
                .headers()
                .get("User-Agent")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_string(),
            method: req.method().to_string(),
            headers: req
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            body: req
                .extensions()
                .get::<Bytes>()
                .and_then(|bytes| String::from_utf8_lossy(bytes).as_ref().parse().ok()),
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: res
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: res.extensions().get::<u64>().cloned().unwrap_or(0),
            load_time: duration.as_secs_f64(),
            body: res
                .extensions()
                .get::<Bytes>()
                .and_then(|bytes| String::from_utf8_lossy(bytes).as_ref().parse().ok()),
        }
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        if !res.status().is_success() {
            let error_info = res
                .extensions()
                .get::<Bytes>()
                .and_then(|bytes| {
                    let body_str = String::from_utf8_lossy(bytes);
                    serde_json::from_str::<Value>(&body_str).ok()
                })
                .map(|value| {
                    let message = match &value {
                        Value::Object(map) => map
                            .get("message")
                            .or_else(|| map.get("error"))
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| value.to_string()),
                        _ => value.to_string(),
                    };

                    vec![ErrorInfo {
                        source: "axum".to_string(),
                        error_type: format!("HTTP_{}", res.status().as_u16()),
                        message,
                        file: String::new(),
                        line: 0,
                    }]
                });

            error_info
        } else {
            None
        }
    }
}
