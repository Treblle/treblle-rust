use chrono::Utc;
use rocket::{Request, Response};
use serde_json::Value;
use std::{sync::RwLock, time::Duration};
use treblle_core::{
    payload::HttpExtractor,
    schema::{ErrorInfo, RequestInfo, ResponseInfo},
};

/// State key for storing request/response bodies
#[derive(Default)]
pub struct TreblleState {
    pub request_body: RwLock<Option<Value>>,
    pub response_body: RwLock<Option<Value>>,
}

pub struct RocketExtractor;

impl HttpExtractor for RocketExtractor {
    type Request = Request<'static>;
    type Response = Response<'static>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        let body = req.rocket()
            .state::<TreblleState>()
            .and_then(|state| state.request_body.read().ok())
            .and_then(|guard| guard.clone());

        RequestInfo {
            timestamp: Utc::now(),
            ip: req.client_ip()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            url: req.uri().to_string(),
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
        // Get content length using Rocket's header access methods
        let size = res.headers()
            .get_one("content-length")
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
        if res.status().class().is_server_error() || res.status().class().is_client_error() {
            Some(vec![ErrorInfo {
                source: "rocket".to_string(),
                error_type: format!("HTTP_{}", res.status().code),
                message: format!("HTTP {} {}",
                                 res.status().code,
                                 res.status().reason().unwrap_or("")
                ),
                file: String::new(),
                line: 0,
            }])
        } else {
            None
        }
    }
}