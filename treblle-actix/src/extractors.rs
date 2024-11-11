use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    HttpMessage,
    web::Bytes,
};
use std::time::Duration;
use std::sync::OnceLock;
use actix_http::body::MessageBody;
use treblle_core::{
    payload::HttpExtractor,
    schema::{RequestInfo, ResponseInfo, ErrorInfo, ServerInfo, OsInfo},
};
use serde_json::Value;
use tracing::warn;
use actix_http::header::HeaderMap as ActixHeaderMap;
use http::HeaderMap as HttpHeaderMap;

pub struct ActixExtractor;

static SERVER_INFO: OnceLock<ServerInfo> = OnceLock::new();

impl ActixExtractor {
    fn get_server_info() -> &'static ServerInfo {
        SERVER_INFO.get_or_init(|| {
            let os_info = os_info::get();
            ServerInfo {
                ip: local_ip_address::local_ip()
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
                timezone: time::UtcOffset::current_local_offset()
                    .map(|o| o.to_string())
                    .unwrap_or_else(|_| "UTC".to_string()),
                software: Some(format!("actix-web/{}", env!("CARGO_PKG_VERSION"))),
                signature: None,
                protocol: "HTTP/1.1".to_string(),
                encoding: None,
                os: OsInfo {
                    name: std::env::consts::OS.to_string(),
                    release: os_info.version().to_string(),
                    architecture: std::env::consts::ARCH.to_string(),
                },
            }
        })
    }

    fn construct_full_url(req: &ServiceRequest) -> String {
        let connection_info = req.connection_info();
        let scheme = connection_info.scheme();
        let host = connection_info.host();
        let uri = req.uri();
        format!("{}://{}{}", scheme, host, uri.path_and_query().map(|p| p.as_str()).unwrap_or(""))
    }

    // Convert Actix HeaderMap to HTTP HeaderMap
    fn convert_headers(headers: &ActixHeaderMap) -> HttpHeaderMap {
        let mut http_headers = HttpHeaderMap::new();
        for (key, value) in headers.iter() {
            if let Ok(name) = http::header::HeaderName::from_bytes(key.as_str().as_bytes()) {
                if let Ok(val) = http::header::HeaderValue::from_bytes(value.as_bytes()) {
                    http_headers.insert(name, val);
                }
            }
        }
        http_headers
    }
}

impl HttpExtractor for ActixExtractor {
    type Request = ServiceRequest;
    type Response = ServiceResponse;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        let headers = Self::convert_headers(req.headers());
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: treblle_core::utils::extract_ip_from_headers(&headers)
                .or_else(|| Some(req.connection_info().realip_remote_addr()?.to_string()))
                .unwrap_or_else(|| "unknown".to_string()),
            url: Self::construct_full_url(req),
            user_agent: req.headers()
                .get("User-Agent")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_string(),
            method: req.method().to_string(),
            headers: req.headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            body: req.request()
                .extensions()
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
        let body_size = match res.response().body().size() {
            actix_http::body::BodySize::None => 0,
            actix_http::body::BodySize::Sized(size) => size,
            actix_http::body::BodySize::Stream => 0, // Can't determine size of streaming body
        };

        ResponseInfo {
            headers: res.headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: body_size,
            load_time: duration.as_secs_f64(),
            body: res.request()
                .extensions()
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
            res.request()
                .extensions()
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
                    let message = match &value {
                        Value::Object(map) => map
                            .get("message")
                            .or_else(|| map.get("error"))
                            .map(|v| match v {
                                Value::String(s) => s.clone(),
                                _ => v.to_string().trim_matches('"').to_string(),
                            })
                            .unwrap_or_else(|| match &value {
                                Value::String(s) => s.clone(),
                                _ => value.to_string().trim_matches('"').to_string(),
                            }),
                        Value::String(s) => s.clone(),
                        _ => value.to_string().trim_matches('"').to_string(),
                    };

                    vec![ErrorInfo {
                        source: "actix".to_string(),
                        error_type: format!("HTTP_{}", res.status().as_u16()),
                        message,
                        file: String::new(),
                        line: 0,
                    }]
                })
        } else {
            None
        }
    }

    fn get_server_info() -> ServerInfo {
        Self::get_server_info().clone()
    }
}