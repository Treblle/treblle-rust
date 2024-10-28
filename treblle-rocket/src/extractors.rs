use rocket::{http::HeaderMap, local::blocking::LocalResponse, Request, Response};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use treblle_core::{
    payload::HttpExtractor,
    schema::{ErrorInfo, RequestInfo, ResponseInfo},
};

#[allow(dead_code)] // Allow because it's used in tests
trait IntoResponse {
    fn as_response(&self) -> Response<'_>;
}

impl<'r> IntoResponse for LocalResponse<'r> {
    fn as_response(&self) -> Response<'_> {
        let mut binding = Response::build();
        let mut response = binding.status(self.status());

        for header in self.headers().iter() {
            response = response.header(header);
        }

        response.finalize()
    }
}

fn extract_headers(headers: &HeaderMap<'_>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|header| (header.name.to_string(), header.value.to_string()))
        .collect()
}

pub struct RocketExtractor<'a>(std::marker::PhantomData<&'a ()>);

impl<'a> HttpExtractor for RocketExtractor<'a> {
    type Request = Request<'a>;
    type Response = Response<'a>;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        let mut info = RequestInfo {
            ip: req.client_ip().map(|ip| ip.to_string()).unwrap_or_default(),
            url: req.uri().to_string(),
            user_agent: req
                .headers()
                .get_one("User-Agent")
                .unwrap_or_default()
                .to_string(),
            method: req.method().to_string(),
            headers: extract_headers(req.headers()),
            body: None,
            timestamp: chrono::Utc::now(),
        };

        if let Some(body) = req.local_cache(|| None::<Vec<u8>>) {
            if let Ok(json) = serde_json::from_slice::<Value>(body) {
                info.body = Some(json);
            }
        }

        info
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        let mut info = ResponseInfo {
            headers: extract_headers(res.headers()),
            code: res.status().code,
            size: 0,
            load_time: duration.as_secs_f64(),
            body: None,
        };

        // Handle response body
        /* if let Some(body_reader) = res.body().to_bytes() {
            let body_bytes = body_reader.into_inner();
            info.size = body_bytes.len();
            if let Ok(json) = serde_json::from_slice::<Value>(&body_bytes) {
                info.body = Some(json);
            }
        } */

        info
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        if res.status().code >= 400 {
            let mut error_info = vec![ErrorInfo {
                source: String::from("http"),
                error_type: format!("HTTP_{}", res.status().code),
                message: res.status().to_string(),
                file: String::new(),
                line: 0,
            }];

            // Try to get error message from body
            /* if let Some(body_reader) = res.body().to_bytes() {
                let body_bytes = body_reader.into_inner();
                if let Ok(json) = serde_json::from_slice::<Value>(&body_bytes) {
                    if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
                        error_info[0].message = message.to_string();
                    }
                }
            } */

            Some(error_info)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::{
        http::{ContentType, Header, Status},
        local::blocking::Client,
        serde::json::json,
    };
    use std::io::Cursor;

    #[test]
    fn test_extract_request_info() {
        let rocket = rocket::build();
        let client = Client::tracked(rocket).expect("valid rocket instance");
        let req = client
            .get("/test")
            .header(Header::new("User-Agent", "test-agent"))
            .header(ContentType::JSON);

        let info = RocketExtractor::extract_request_info(req.inner());

        assert_eq!(info.url, "/test");
        assert_eq!(info.user_agent, "test-agent");
        assert_eq!(info.method, "GET");
    }

    #[test]
    fn test_extract_response_info() {
        let rocket = rocket::build();
        let client = Client::tracked(rocket).expect("valid rocket instance");
        let local_response = client.get("/").dispatch();
        let response = local_response.as_response();

        let info = RocketExtractor::extract_response_info(&response, Duration::from_secs(1));

        assert_eq!(info.code, 404);
        assert_eq!(info.load_time, 1.0);
    }

    #[test]
    fn test_extract_error_info() {
        let error_body = json!({
            "error": "Not Found",
            "message": "Resource does not exist"
        });

        let response = Response::build()
            .status(Status::NotFound)
            .header(ContentType::JSON)
            .sized_body(
                error_body.to_string().len(),
                Cursor::new(error_body.to_string()),
            )
            .finalize();

        let errors = RocketExtractor::extract_error_info(&response).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, "HTTP_404");
        assert!(errors[0].message.contains("Resource does not exist"));
    }

    #[test]
    fn test_empty_body_handling() {
        let response = Response::build()
            .status(Status::Ok)
            .header(ContentType::JSON)
            .sized_body(2, Cursor::new("{}"))
            .finalize();

        let info = RocketExtractor::extract_response_info(&response, Duration::from_secs(1));
        assert!(info.body.is_none());
    }

    #[test]
    fn test_invalid_json_body() {
        let response = Response::build()
            .status(Status::BadRequest)
            .header(ContentType::JSON)
            .sized_body(11, Cursor::new("invalid json"))
            .finalize();

        let info = RocketExtractor::extract_response_info(&response, Duration::from_secs(1));
        assert!(info.body.is_none());
    }
}
