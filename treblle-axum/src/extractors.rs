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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use serde_json::json;
    use std::time::Duration;

    fn create_test_request() -> Request<Body> {
        let mut req = Request::builder()
            .uri("https://api.example.com/test")
            .method("POST")
            .header("User-Agent", "test-agent")
            .header("Content-Type", "application/json")
            .body(Body::empty())
            .unwrap();

        let body = json!({
            "test": "value",
            "password": "secret"
        });
        req.extensions_mut().insert(Bytes::from(body.to_string()));
        req
    }

    fn create_test_response(status: StatusCode, body: Value) -> Response<Body> {
        let mut res = Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(Bytes::from(body.to_string()));
        res
    }

    #[test]
    fn test_extract_request_info() {
        let req = create_test_request();
        let info = AxumExtractor::extract_request_info(&req);

        assert_eq!(info.url, "https://api.example.com/test");
        assert_eq!(info.method, "POST");
        assert_eq!(info.user_agent, "test-agent");
        assert!(info.body.is_some());
    }

    #[test]
    fn test_extract_response_info() {
        let res = create_test_response(StatusCode::OK, json!({"result": "success"}));
        let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));

        assert_eq!(info.code, 200);
        assert_eq!(info.load_time, 1.0);
        assert!(info.body.is_some());
    }

    #[test]
    fn test_extract_error_info() {
        let error_body = json!({
            "error": "Not Found",
            "message": "Resource does not exist"
        });
        let res = create_test_response(StatusCode::NOT_FOUND, error_body);

        let errors = AxumExtractor::extract_error_info(&res).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, "HTTP_404");
        assert!(errors[0].message.contains("Resource does not exist"));
    }

    #[test]
    fn test_empty_body_handling() {
        let mut res = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(Bytes::new());

        let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert!(info.body.is_none());
    }

    #[test]
    fn test_invalid_json_body() {
        let mut res = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(Bytes::from("invalid json"));

        let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert!(info.body.is_none());
    }
}
