use crate::utils::{hashmap_to_json_value, json_value_to_hashmap, mask_sensitive_data};
use crate::Config;
use crate::{
    extractors::TreblleExtractor,
    schema::{
        ErrorInfo, LanguageInfo, PayloadData, RequestInfo, ResponseInfo, ServerInfo, TrebllePayload,
    },
};
use serde_json::Value;
use std::time::Duration;

pub struct PayloadBuilder;

impl PayloadBuilder {
    fn process_errors(
        response_info: &ResponseInfo,
        extracted_errors: Option<Vec<ErrorInfo>>,
    ) -> Vec<ErrorInfo> {
        let mut errors = Vec::new();

        // Add errors based on status code
        if response_info.code >= 400 {
            // Try to extract error from response body
            if let Some(body) = &response_info.body {
                let error_message = match body {
                    Value::Object(map) => {
                        // Common error field names
                        ["error", "message", "error_description"]
                            .iter()
                            .find_map(|&key| map.get(key))
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| body.to_string())
                    }
                    _ => body.to_string(),
                };

                errors.push(ErrorInfo {
                    source: "http".to_string(),
                    error_type: format!("HTTP_{}", response_info.code),
                    message: error_message,
                    file: String::new(),
                    line: 0,
                });
            }
        }

        // Add any framework-specific errors
        if let Some(mut extracted) = extracted_errors {
            errors.append(&mut extracted);
        }

        errors
    }

    pub fn build_request_payload<E: TreblleExtractor>(
        req: &E::Request,
        config: &Config,
    ) -> TrebllePayload {
        let mut request_info = E::extract_request_info(req);

        // Convert headers to Value, mask, and convert back
        let headers_value = hashmap_to_json_value(&request_info.headers);
        let masked_headers =
            mask_sensitive_data(&headers_value, &config.masked_fields_regex, &config.masked_fields);

        request_info.headers = json_value_to_hashmap(masked_headers);

        // Mask body if present
        if let Some(body) = request_info.body.as_ref() {
            request_info.body =
                Some(mask_sensitive_data(body, &config.masked_fields_regex, &config.masked_fields));
        }

        TrebllePayload {
            api_key: config.api_key.clone(),
            project_id: config.project_id.clone(),
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData {
                server: ServerInfo::default(),
                language: LanguageInfo {
                    name: "rust".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                request: request_info,
                response: ResponseInfo::default(),
                errors: Vec::new(),
            },
        }
    }

    pub fn build_response_payload<E: TreblleExtractor>(
        res: &E::Response,
        config: &Config,
        duration: Duration,
    ) -> TrebllePayload {
        let mut response_info = E::extract_response_info(res, duration);

        // Convert headers to Value, mask, and convert back
        let headers_value = hashmap_to_json_value(&response_info.headers);
        let masked_headers =
            mask_sensitive_data(&headers_value, &config.masked_fields_regex, &config.masked_fields);
        response_info.headers = json_value_to_hashmap(masked_headers);

        // Mask body if present
        if let Some(body) = response_info.body.as_ref() {
            response_info.body =
                Some(mask_sensitive_data(body, &config.masked_fields_regex, &config.masked_fields));
        }

        // Extract and process errors
        let errors = Self::process_errors(&response_info, E::extract_error_info(res));

        TrebllePayload {
            api_key: config.api_key.clone(),
            project_id: config.project_id.clone(),
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData {
                server: ServerInfo::default(),
                language: LanguageInfo {
                    name: "rust".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                request: RequestInfo::default(),
                response: response_info,
                errors,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::schema::OsInfo;

    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MockResponse {
        status_code: u16,
        body: Option<Value>,
        error_info: Option<Vec<ErrorInfo>>,
    }

    struct MockExtractor;

    impl TreblleExtractor for MockExtractor {
        type Request = ();
        type Response = MockResponse;

        fn extract_server_info() -> ServerInfo {
            ServerInfo {
                ip: "127.0.0.1".to_string(),
                timezone: "UTC".to_string(),
                software: Some("mock-server/1.0".to_string()),
                signature: None,
                protocol: "HTTP/1.1".to_string(),
                encoding: None,
                os: OsInfo {
                    name: "mock-os".to_string(),
                    release: "1.0".to_string(),
                    architecture: "mock64".to_string(),
                },
            }
        }

        fn extract_request_info(_req: &Self::Request) -> RequestInfo {
            let mut headers = HashMap::new();
            headers.insert("password".to_string(), "secret123".to_string());
            headers.insert("content-type".to_string(), "application/json".to_string());

            RequestInfo {
                headers,
                body: Some(json!({
                    "password": "secret123",
                    "email": "test@example.com"
                })),
                ..Default::default()
            }
        }

        fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
            ResponseInfo {
                code: res.status_code,
                body: res.body.clone(),
                load_time: duration.as_secs_f64(),
                ..Default::default()
            }
        }

        fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
            res.error_info.clone()
        }
    }

    #[test]
    fn test_build_request_payload_with_sensitive_data() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let payload = PayloadBuilder::build_request_payload::<MockExtractor>(&(), &config);

        assert_eq!(payload.api_key, "test_key");
        assert_eq!(payload.data.request.headers.get("password").unwrap(), "*****");
        assert_eq!(payload.data.request.body.as_ref().unwrap()["password"], "*****");
        assert_eq!(payload.data.request.body.as_ref().unwrap()["email"], "test@example.com");
    }

    #[test]
    fn test_build_response_payload_success() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let response = MockResponse {
            status_code: 200,
            body: Some(json!({"result": "success"})),
            ..Default::default()
        };

        let payload = PayloadBuilder::build_response_payload::<MockExtractor>(
            &response,
            &config,
            Duration::from_secs(1),
        );

        assert_eq!(payload.data.response.code, 200);
        assert!(payload.data.errors.is_empty());
    }

    #[test]
    fn test_build_response_payload_with_http_error() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let response = MockResponse {
            status_code: 404,
            body: Some(json!({
                "error": "Resource not found",
                "details": "The requested resource does not exist"
            })),
            ..Default::default()
        };

        let payload = PayloadBuilder::build_response_payload::<MockExtractor>(
            &response,
            &config,
            Duration::from_secs(1),
        );

        assert_eq!(payload.data.response.code, 404);
        assert_eq!(payload.data.errors.len(), 1);
        assert_eq!(payload.data.errors[0].error_type, "HTTP_404");
        assert!(payload.data.errors[0].message.contains("Resource not found"));
    }

    #[test]
    fn test_build_response_payload_with_framework_error() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let response = MockResponse {
            status_code: 500,
            body: Some(json!({"status": "error"})),
            error_info: Some(vec![ErrorInfo {
                source: "framework".to_string(),
                error_type: "RuntimeError".to_string(),
                message: "Internal server error".to_string(),
                file: "handler.rs".to_string(),
                line: 42,
            }]),
        };

        let payload = PayloadBuilder::build_response_payload::<MockExtractor>(
            &response,
            &config,
            Duration::from_secs(1),
        );

        assert_eq!(payload.data.errors.len(), 2); // HTTP error + framework error
        assert!(payload.data.errors.iter().any(|e| e.source == "framework"));
        assert!(payload.data.errors.iter().any(|e| e.source == "http"));
    }

    #[test]
    fn test_build_response_payload_with_sensitive_data() {
        let config =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let response = MockResponse {
            status_code: 200,
            body: Some(json!({
                "user": {
                    "email": "test@example.com",
                    "password": "secret123",
                    "api_key": "test_key",
                    "credit_card": {
                        "card_number": "4111-1111-1111-1111",
                        "card_cvv": "123"
                    },
                    "ssn": "123-45-6789"
                }
            })),
            ..Default::default()
        };

        let payload = PayloadBuilder::build_response_payload::<MockExtractor>(
            &response,
            &config,
            Duration::from_secs(1),
        );

        let response_body = payload.data.response.body.unwrap();

        assert_eq!(response_body["user"]["password"], "*****");
        assert_eq!(response_body["user"]["api_key"], "*****");
        assert_eq!(response_body["user"]["credit_card"]["card_number"], "*****");
        assert_eq!(response_body["user"]["credit_card"]["card_cvv"], "*****");
        assert_eq!(response_body["user"]["ssn"], "*****");
        assert_eq!(response_body["user"]["email"], "test@example.com");
    }
}
