#[cfg(test)]
mod tests {
    use crate::extractors::AxumExtractor;
    use crate::{AxumConfig, Treblle, TreblleLayer};
    use axum::body::{to_bytes, Body};
    use axum::response::Response;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use http::header::CONTENT_TYPE;
    use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
    use hyper::body::Bytes;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use std::time::Duration;
    use tower::{ServiceBuilder, ServiceExt};
    use tower_http::timeout::TimeoutLayer;
    use treblle_core::constants::MAX_BODY_SIZE;
    use treblle_core::extractors::TreblleExtractor;
    use treblle_core::PayloadBuilder;

    fn create_test_request(headers: Vec<(&str, &str)>) -> http::Request<Body> {
        let builder = http::Request::builder()
            .uri("https://api.example.com/test")
            .method("POST");

        let headers_map = headers
            .into_iter()
            .map(|(name, value)| {
                (
                    HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    HeaderValue::from_str(value).unwrap(),
                )
            })
            .collect::<HeaderMap>();

        let req = builder.body(Body::empty()).unwrap();
        let (mut parts, body) = req.into_parts();
        parts.headers = headers_map;
        http::Request::from_parts(parts, body)
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
    fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .add_masked_fields(vec!["password".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle
            .config
            .core
            .masked_fields
            .iter()
            .any(|r| r.as_str().contains("password")));
        assert!(treblle
            .config
            .core
            .ignored_routes
            .iter()
            .any(|r| r.as_str().contains("/health")));
    }

    #[test]
    fn test_axum_config() {
        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config.add_masked_fields(vec!["password".to_string()]);
        config.add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config
            .core
            .masked_fields
            .iter()
            .any(|r| r.as_str().contains("password")));
        assert!(config
            .core
            .ignored_routes
            .iter()
            .any(|r| r.as_str().contains("/health")));
    }

    #[test]
    fn test_ip_extraction() {
        let test_cases = vec![
            (vec![("X-Forwarded-For", "203.0.113.195")], "203.0.113.195"),
            (vec![("X-Real-IP", "203.0.113.196")], "203.0.113.196"),
            (
                vec![("Forwarded", "for=192.0.2.60;proto=http;by=203.0.113.43")],
                "192.0.2.60",
            ),
            (
                vec![
                    ("X-Forwarded-For", "203.0.113.195"),
                    ("X-Real-IP", "203.0.113.196"),
                ],
                "203.0.113.195", // X-Forwarded-For should take precedence
            ),
            (
                vec![
                    ("Forwarded", "for=192.0.2.60"),
                    ("X-Forwarded-For", "203.0.113.195"),
                ],
                "192.0.2.60", // Forwarded header should take precedence
            ),
            (vec![], "unknown"), // No IP headers present
        ];

        for (headers, expected_ip) in test_cases {
            let req = create_test_request(headers);
            let info = AxumExtractor::extract_request_info(&req);
            assert_eq!(info.ip, expected_ip);
        }
    }

    #[test]
    fn test_response_size_calculation() {
        let body = json!({
            "key": "value",
            "nested": {
                "array": [1, 2, 3]
            }
        });
        let body_bytes = Bytes::from(body.to_string());
        let expected_size = body_bytes.len() as u64;

        let mut res = http::Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(body_bytes);

        let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert_eq!(info.size, expected_size);
    }

    #[test]
    fn test_url_construction() {
        let test_cases = vec![
            (
                vec![("Host", "api.example.com")],
                "https://api.example.com/test?query=value",
                "https://api.example.com/test?query=value",
            ),
            (
                vec![],
                "/test",
                "http:///test", // No host header
            ),
            (
                vec![("Host", "localhost:8080")],
                "/api/v1/users",
                "http://localhost:8080/api/v1/users",
            ),
        ];

        for (headers, uri, expected_url) in test_cases {
            let mut req = create_test_request(headers);
            *req.uri_mut() = uri.parse().unwrap();

            let info = AxumExtractor::extract_request_info(&req);
            assert_eq!(info.url, expected_url);
        }
    }

    #[test]
    fn test_extract_request_info() {
        let req = create_test_request(vec![
            ("Host", "api.example.com"),
            ("User-Agent", "test-agent"),
        ]);
        let info = AxumExtractor::extract_request_info(&req);

        assert_eq!(info.url, "https://api.example.com/test");
        assert_eq!(info.method, "POST");
        assert_eq!(info.user_agent, "test-agent");
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
    fn test_error_extraction() {
        let test_cases = vec![
            (
                StatusCode::NOT_FOUND,
                json!({
                    "error": "Resource not found",
                    "message": "The requested user does not exist"
                }),
                "The requested user does not exist",
            ),
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "message": "Invalid input"
                }),
                "Invalid input",
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!("Unexpected server error"),
                "Unexpected server error",
            ),
        ];

        for (status, error_body, expected_message) in test_cases {
            let mut res = http::Response::builder()
                .status(status)
                .body(Body::empty())
                .unwrap();

            res.extensions_mut()
                .insert(Bytes::from(error_body.to_string()));

            let errors = AxumExtractor::extract_error_info(&res).unwrap();
            assert_eq!(errors[0].source, "axum");
            assert_eq!(errors[0].error_type, format!("HTTP_{}", status.as_u16()));
            assert_eq!(
                errors[0].message, expected_message,
                "Failed for status code {} with body {:?}",
                status, error_body
            );
        }
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
    fn test_large_response_body() {
        let large_body = vec![0u8; 10 * 1024 * 1024]; // 10MB
        let body_bytes = Bytes::from(large_body);
        let expected_size = body_bytes.len() as u64;

        let mut res = http::Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();

        res.extensions_mut().insert(body_bytes);

        let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));
        assert_eq!(info.size, expected_size);
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

    // Simple echo handler - doesn't do any masking
    async fn echo_handler(Json(payload): Json<Value>) -> Json<Value> {
        Json(payload)
    }

    async fn plain_text_handler() -> (StatusCode, &'static str) {
        (StatusCode::OK, "Hello, World!")
    }

    // Helper function to recursively find a field value in a JSON object
    fn find_field_value<'a>(json: &'a Value, field: &str) -> Option<&'a str> {
        match json {
            Value::Object(map) => {
                for (key, value) in map {
                    if key == field {
                        return value.as_str();
                    }
                    if let Some(found) = find_field_value(value, field) {
                        return Some(found);
                    }
                }
                None
            }
            Value::Array(arr) => {
                for value in arr {
                    if let Some(found) = find_field_value(value, field) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn setup_test_app() -> Router {
        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config.core.add_masked_fields(vec![
            "password".to_string(),
            "credit_card".to_string(),
            "cvv".to_string(),
            "ssn".to_string(),
        ]);

        let layer = Arc::new(TreblleLayer::new(config));

        Router::new()
            .route("/echo", post(echo_handler))
            .route("/text", get(plain_text_handler))
            .layer(ServiceBuilder::new().layer(TimeoutLayer::new(Duration::from_secs(5))))
            .with_state(layer)
    }

    #[tokio::test]
    async fn test_middleware_preserves_original_data() {
        let app = setup_test_app();

        let test_data = json!({
            "user": {
                "email": "test@example.com",
                "password": "secret123",
                "credit_card": "4111-1111-1111-1111"
            }
        });

        let request = http::Request::builder()
            .uri("/echo")
            .method(Method::POST)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(test_data.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify the response contains unmasked data (original data preserved)
        let body_bytes = to_bytes(response.into_body(), MAX_BODY_SIZE).await.unwrap();
        let body: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["password"], "secret123"); // Password not masked in response
        assert_eq!(body["user"]["credit_card"], "4111-1111-1111-1111"); // CC not masked in response
    }

    #[tokio::test]
    async fn test_middleware_allows_non_json_requests() {
        let app = setup_test_app();

        let request = http::Request::builder()
            .uri("/text")
            .method(Method::GET)
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Get the response body
        let body_bytes = to_bytes(response.into_body(), MAX_BODY_SIZE).await.unwrap();
        assert_eq!(&body_bytes[..], b"Hello, World!");
    }

    #[tokio::test]
    async fn test_middleware_respects_ignored_routes() {
        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_ignored_routes(vec!["/ignored.*".to_string()]);

        let app = Router::new()
            .route("/ignored", post(echo_handler))
            .layer(ServiceBuilder::new().layer(TimeoutLayer::new(Duration::from_secs(5))))
            .with_state(Arc::new(TreblleLayer::new(config)));

        let test_data = json!({
            "password": "secret123",
            "credit_card": "4111-1111-1111-1111"
        });

        let request = http::Request::builder()
            .uri("/ignored")
            .method(Method::POST)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(test_data.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify data is unmodified for ignored routes
        let body_bytes = to_bytes(response.into_body(), MAX_BODY_SIZE).await.unwrap();
        let body: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["password"], "secret123");
        assert_eq!(body["credit_card"], "4111-1111-1111-1111");
    }

    #[tokio::test]
    async fn test_treblle_payload_creation() {
        use axum::body::Bytes;

        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config.core.add_masked_fields(vec!["password".to_string()]);

        let test_data = json!({
            "username": "test_user",
            "password": "secret123"
        });

        // Create a request with sensitive data
        let mut req = http::Request::new(Body::empty());
        *req.body_mut() = Body::from(test_data.to_string());
        req.extensions_mut()
            .insert(Bytes::from(test_data.to_string()));

        // Create payload using the same mechanism as middleware
        let payload = PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

        // Verify the payload has masked sensitive data
        if let Some(body) = &payload.data.request.body {
            if let Some(password) = body.get("password") {
                assert_eq!(password.as_str().unwrap(), "*****");
            }
            if let Some(username) = body.get("username") {
                assert_eq!(username.as_str().unwrap(), "test_user");
            }
        }
    }

    #[tokio::test]
    async fn test_data_masking_patterns() {
        let app = setup_test_app();

        let test_cases = vec![
            // Test case-insensitive password masking
            (
                json!({
                    "Password": "secret123",
                    "password": "secret456",
                    "user_password": "secret789"
                }),
                vec!["Password", "password", "user_password"],
            ),
            // Test nested object masking
            (
                json!({
                    "user": {
                        "password": "secret123",
                        "credit_card": "4111-1111-1111-1111",
                        "profile": {
                            "ssn": "123-45-6789"
                        }
                    }
                }),
                vec!["password", "credit_card", "ssn"],
            ),
            // Test array masking
            (
                json!({
                    "users": [
                        {"password": "secret1", "credit_card": "4111-1111-1111-1111"},
                        {"password": "secret2", "credit_card": "4222-2222-2222-2222"}
                    ]
                }),
                vec!["password", "credit_card"],
            ),
            // Test custom field patterns
            (
                json!({
                    "api_key": "sk_test_123",
                    "stripe_secret": "sk_live_456",
                    "custom_secret_field": "sensitive_data"
                }),
                vec!["api_key", "stripe_secret", "custom_secret_field"],
            ),
        ];

        for (payload, fields_to_check) in test_cases {
            let payload_bytes = payload.to_string();

            // Create request for testing the Treblle payload
            let req = http::Request::builder()
                .uri("/echo")
                .method(Method::POST)
                .header(CONTENT_TYPE, "application/json")
                .extension(bytes::Bytes::from(payload_bytes.clone()))
                .body(Body::empty())
                .unwrap();

            // Create config with all fields that should be masked
            let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
            config.core.add_masked_fields(vec![
                "password".to_string(),
                "Password".to_string(),
                "user_password".to_string(),
                "credit_card".to_string(),
                "ssn".to_string(),
                "api_key".to_string(),
                "stripe_secret".to_string(),
                "custom_secret_field".to_string(),
            ]);

            let treblle_payload =
                PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

            // Verify the payload has masked sensitive data
            if let Some(body) = treblle_payload.data.request.body {
                for &field in &fields_to_check {
                    if let Some(value) = find_field_value(&body, field) {
                        assert_eq!(value, "*****", "Field '{}' was not properly masked", field);
                    }
                }
            }

            // Create a new request for testing the response
            let response_req = http::Request::builder()
                .uri("/echo")
                .method(Method::POST)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(payload_bytes))
                .unwrap();

            // Verify that the original response is unmasked
            let response = app.clone().oneshot(response_req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);

            let body_bytes = to_bytes(response.into_body(), MAX_BODY_SIZE).await.unwrap();
            let response_body: Value = serde_json::from_slice(&body_bytes).unwrap();

            // Original data should be preserved in the response
            for &field in &fields_to_check {
                if let Some(original_value) = find_field_value(&payload, field) {
                    if let Some(response_value) = find_field_value(&response_body, field) {
                        assert_eq!(
                            original_value, response_value,
                            "Field '{}' should not be masked in the response",
                            field
                        );
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn test_partial_field_masking() {
        let app = setup_test_app();

        let test_data = json!({
            "user": {
                "email": "test@example.com",
                "credit_card": {
                    "number": "4111-1111-1111-1111",
                    "expiry": "12/24",  // Should not be masked
                    "cvv": "123"  // Should be masked
                },
                "shipping_address": {  // Should not be masked
                    "street": "123 Main St",
                    "city": "Test City"
                }
            }
        });

        let payload_bytes = test_data.to_string();

        // Create request for testing the Treblle payload
        let req = http::Request::builder()
            .uri("/echo")
            .method(Method::POST)
            .header(CONTENT_TYPE, "application/json")
            .extension(bytes::Bytes::from(payload_bytes.clone()))
            .body(Body::empty())
            .unwrap();

        // Create config with specific fields that should be masked
        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec!["number".to_string(), "cvv".to_string()]);

        let treblle_payload =
            PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

        // Verify masking in Treblle payload
        if let Some(body) = treblle_payload.data.request.body {
            assert_eq!(body["user"]["email"].as_str().unwrap(), "test@example.com");
            assert_eq!(
                body["user"]["credit_card"]["number"].as_str().unwrap(),
                "*****"
            );
            assert_eq!(
                body["user"]["credit_card"]["expiry"].as_str().unwrap(),
                "12/24"
            );
            assert_eq!(
                body["user"]["credit_card"]["cvv"].as_str().unwrap(),
                "*****"
            );
            assert_eq!(
                body["user"]["shipping_address"]["street"].as_str().unwrap(),
                "123 Main St"
            );
        }

        // Create a new request for testing the response
        let response_req = http::Request::builder()
            .uri("/echo")
            .method(Method::POST)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(payload_bytes))
            .unwrap();

        // Verify original response is unmasked
        let response = app.oneshot(response_req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), MAX_BODY_SIZE).await.unwrap();
        let body: Value = serde_json::from_slice(&body_bytes).unwrap();

        // Original data should be preserved in the response
        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["credit_card"]["number"], "4111-1111-1111-1111");
        assert_eq!(body["user"]["credit_card"]["expiry"], "12/24");
        assert_eq!(body["user"]["credit_card"]["cvv"], "123");
        assert_eq!(body["user"]["shipping_address"]["street"], "123 Main St");
    }
}
