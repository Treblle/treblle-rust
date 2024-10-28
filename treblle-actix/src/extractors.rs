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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, http::header, HttpResponse};
    use serde_json::json;

    #[actix_web::test]
    async fn test_extract_request_info() {
        let _app = test::init_service(
            actix_web::App::new()
                .default_service(actix_web::web::to(|| async { HttpResponse::Ok().finish() }))
        ).await;

        let payload = json!({
            "test": "value",
            "nested": {
                "field": "data"
            }
        });

        let req = test::TestRequest::default()
            .insert_header((header::USER_AGENT, "test-agent"))
            .insert_header((header::CONTENT_TYPE, "application/json"))
            .uri("/test")
            .to_srv_request();

        req.request().extensions_mut()
            .insert(Bytes::from(payload.to_string()));

        let info = ActixExtractor::extract_request_info(&req);

        assert_eq!(info.url, "/test");
        assert_eq!(info.user_agent, "test-agent");
        assert!(info.body.is_some());
        if let Some(body) = info.body {
            assert_eq!(body["test"], "value");
            assert_eq!(body["nested"]["field"], "data");
        }
    }

    #[actix_web::test]
    async fn test_extract_response_info() {
        let json_data = json!({
            "result": "success",
            "data": {
                "id": 1,
                "value": "test"
            }
        });

        let req = test::TestRequest::default().to_http_request();
        req.extensions_mut().insert(Bytes::from(json_data.to_string()));

        let res = ServiceResponse::new(
            req,
            HttpResponse::Ok()
                .content_type("application/json")
                .insert_header((header::CONTENT_LENGTH, json_data.to_string().len().to_string()))
                .body(json_data.to_string())
        );

        let info = ActixExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert_eq!(info.code, 200);
        assert_eq!(info.load_time, 1.0);
        assert!(info.body.is_some());
        if let Some(body) = info.body {
            assert_eq!(body["result"], "success");
            assert_eq!(body["data"]["id"], 1);
        }
    }

    #[actix_web::test]
    async fn test_extract_error_info() {
        let error_body = json!({
            "error": "Not Found",
            "message": "Resource does not exist",
            "details": {
                "id": "missing"
            }
        });

        let req = test::TestRequest::default().to_http_request();
        req.extensions_mut().insert(Bytes::from(error_body.to_string()));

        let res = ServiceResponse::new(
            req,
            HttpResponse::NotFound()
                .content_type("application/json")
                .body(error_body.to_string())
        );

        let errors = ActixExtractor::extract_error_info(&res).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, "HTTP_404");
        assert!(errors[0].message.contains("Resource does not exist"));
    }

    #[actix_web::test]
    async fn test_empty_body_handling() {
        let req = test::TestRequest::default().to_http_request();
        req.extensions_mut().insert(Bytes::new());

        let res = ServiceResponse::new(
            req,
            HttpResponse::Ok()
                .content_type("application/json")
                .body("{}")
        );

        let info = ActixExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert!(info.body.is_none());
    }

    #[actix_web::test]
    async fn test_invalid_json_body() {
        let req = test::TestRequest::default().to_http_request();
        req.extensions_mut().insert(Bytes::from("invalid json"));

        let res = ServiceResponse::new(
            req,
            HttpResponse::BadRequest()
                .content_type("application/json")
                .body("invalid json")
        );

        let info = ActixExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert!(info.body.is_none());
    }
}