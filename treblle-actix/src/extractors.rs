use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    HttpMessage,
    web::Bytes,
};
use std::time::Duration;
use treblle_core::payload::HttpExtractor;
use treblle_core::schema::{RequestInfo, ResponseInfo, ErrorInfo};
use serde_json::Value;
use tracing::warn;

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
            // Get the body from payload data if available
            body: req.request().extensions()
                .get::<Bytes>()
                .and_then(|bytes| {
                    if bytes.is_empty() {
                        None
                    } else {
                        serde_json::from_slice(bytes).ok()
                    }
                }),
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        ResponseInfo {
            headers: res.headers().iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: res.response().headers()
                .get(actix_web::http::header::CONTENT_LENGTH)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            load_time: duration.as_secs_f64(),
            // Get the body from extensions if available
            body: res.request().extensions()
                .get::<Bytes>()
                .and_then(|bytes| {
                    if bytes.is_empty() {
                        None
                    } else {
                        serde_json::from_slice(bytes).ok()
                    }
                }),
        }
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        if !res.status().is_success() {
            res.request().extensions()
                .get::<Bytes>()
                .and_then(|bytes| {
                    if bytes.is_empty() {
                        None
                    } else {
                        serde_json::from_slice::<Value>(bytes)
                            .map_err(|e| {
                                warn!("Failed to parse error response body: {}", e);
                                e
                            })
                            .ok()
                    }
                })
                .map(|value| {
                    match &value {
                        Value::Object(map) => {
                            let message = map.get("message")
                                .or_else(|| map.get("error"))
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| value.to_string());

                            vec![ErrorInfo {
                                source: "actix".to_string(),
                                error_type: format!("HTTP_{}", res.status().as_u16()),
                                message,
                                file: String::new(),
                                line: 0,
                            }]
                        },
                        _ => vec![ErrorInfo {
                            source: "actix".to_string(),
                            error_type: format!("HTTP_{}", res.status().as_u16()),
                            message: value.to_string(),
                            file: String::new(),
                            line: 0,
                        }],
                    }
                })
        } else {
            None
        }
    }
}