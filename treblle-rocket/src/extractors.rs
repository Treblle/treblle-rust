use chrono::Utc;
use rocket::{Request, Response};
use serde_json::Value;
use std::{sync::RwLock, time::Duration, sync::OnceLock};
use treblle_core::{
    payload::HttpExtractor,
    schema::{ErrorInfo, RequestInfo, ResponseInfo, ServerInfo, OsInfo},
    utils::extract_ip_from_headers,
};
use http::HeaderMap;
use rocket::http::Status;

static SERVER_INFO: OnceLock<ServerInfo> = OnceLock::new();

/// State key for storing request/response bodies
#[derive(Default)]
pub struct TreblleState {
    pub request_body: RwLock<Option<Value>>,
    pub response_body: RwLock<Option<Value>>,
}

pub struct RocketExtractor;

impl RocketExtractor {
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
                software: Some(format!("rocket/{}", env!("CARGO_PKG_VERSION"))),
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

    fn construct_full_url(req: &Request<'_>) -> String {
        let scheme = if req.headers().get_one("X-Forwarded-Proto").map(|h| h == "https").unwrap_or(false) {
            "https"
        } else {
            "http"
        };

        let host = req.headers().get_one("Host").unwrap_or("localhost");
        format!("{}://{}{}", scheme, host, req.uri())
    }

    fn convert_headers_to_http(headers: &rocket::http::HeaderMap<'_>) -> HeaderMap {
        let mut http_headers = HeaderMap::new();
        for header in headers.iter() {
            if let (Ok(name), Ok(value)) = (
                http::header::HeaderName::from_bytes(header.name.as_str().as_bytes()),
                http::header::HeaderValue::from_str(&header.value.to_string())
            ) {
                http_headers.insert(name, value);
            }
        }
        http_headers
    }

    fn is_error_status(status: Status) -> bool {
        status.code >= 400
    }
}

impl HttpExtractor for RocketExtractor {
    type Request = Request<'static>;
    type Response = Response<'static>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        let headers = Self::convert_headers_to_http(req.headers());

        // Use rocket() to get access to state in Rocket 0.5
        let body = req.rocket()
            .state::<TreblleState>()
            .and_then(|state| state.request_body.read().ok())
            .and_then(|guard| guard.clone());

        RequestInfo {
            timestamp: Utc::now(),
            ip: extract_ip_from_headers(&headers)
                .or_else(|| req.client_ip().map(|addr| addr.to_string()))
                .unwrap_or_else(|| "unknown".to_string()),
            url: Self::construct_full_url(req),
            user_agent: req.headers().get_one("User-Agent")
                .unwrap_or("")
                .to_string(),
            method: req.method().to_string(),
            headers: req.headers().iter()
                .map(|h| (h.name.to_string(), h.value.to_string()))
                .collect(),
            body,
        }
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        let size = res.headers()
            .get_one("Content-Length")
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(0);

        ResponseInfo {
            headers: res.headers().iter()
                .map(|h| (h.name.to_string(), h.value.to_string()))
                .collect(),
            code: res.status().code,
            size,
            load_time: duration.as_secs_f64(),
            body: None,
        }
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        if Self::is_error_status(res.status()) {
            // For Rocket responses, we'll use status code and reason
            let message = format!("HTTP {} {}",
                                  res.status().code,
                                  res.status().reason().unwrap_or(""));

            Some(vec![ErrorInfo {
                source: "rocket".to_string(),
                error_type: format!("HTTP_{}", res.status().code),
                message,
                file: String::new(),
                line: 0,
            }])
        } else {
            None
        }
    }

    fn get_server_info() -> ServerInfo {
        Self::get_server_info().clone()
    }
}