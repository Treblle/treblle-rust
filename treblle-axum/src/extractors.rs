use axum::body::Body;
use axum::http::{Request, Response};
use hyper::body::Bytes;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use treblle_core::{
    extractors::TreblleExtractor,
    schema::{ErrorInfo, OsInfo, RequestInfo, ResponseInfo, ServerInfo},
    utils::extract_ip_from_headers,
};

pub struct AxumExtractor;

static SERVER_INFO: OnceLock<ServerInfo> = OnceLock::new();

impl AxumExtractor {
    fn construct_full_url(req: &Request<Body>) -> String {
        let scheme = req.uri().scheme_str().unwrap_or("http");
        let host = req.headers().get("host").and_then(|h| h.to_str().ok()).unwrap_or("");
        let path_and_query = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("");

        format!("{}://{}{}", scheme, host, path_and_query)
    }
}

impl TreblleExtractor for AxumExtractor {
    type Request = Request<Body>;
    type Response = Response<Body>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        RequestInfo {
            timestamp: chrono::Utc::now(),
            ip: extract_ip_from_headers(req.headers()).unwrap_or_else(|| "unknown".to_string()),
            url: Self::construct_full_url(req),
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
        let body_size = res.extensions().get::<Bytes>().map(|b| b.len() as u64).unwrap_or(0);

        ResponseInfo {
            headers: res
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            code: res.status().as_u16(),
            size: body_size,
            load_time: duration.as_secs_f64(),
            body: res
                .extensions()
                .get::<Bytes>()
                .and_then(|bytes| String::from_utf8_lossy(bytes).as_ref().parse().ok()),
        }
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        if !res.status().is_success() {
            res.extensions()
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
                        source: "axum".to_string(),
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

    fn extract_server_info() -> ServerInfo {
        SERVER_INFO
            .get_or_init(|| {
                let os_info = os_info::get();
                ServerInfo {
                    ip: local_ip_address::local_ip()
                        .map(|ip| ip.to_string())
                        .unwrap_or_else(|_| "unknown".to_string()),
                    timezone: time::UtcOffset::current_local_offset()
                        .map(|o| o.to_string())
                        .unwrap_or_else(|_| "UTC".to_string()), // Provide default timezone
                    software: Some(format!("axum/{}", env!("CARGO_PKG_VERSION"))),
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
            .clone()
    }
}
