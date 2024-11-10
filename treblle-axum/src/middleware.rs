use crate::config::AxumConfig;
use crate::extractors::AxumExtractor;
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response},
    middleware::Next,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error};
use treblle_core::{payload::PayloadBuilder, TreblleClient};
use treblle_core::constants::MAX_BODY_SIZE;

#[derive(Clone)]
pub struct TreblleLayer {
    config: Arc<AxumConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleLayer {
    pub fn new(config: AxumConfig) -> Self {
        TreblleLayer {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone()).expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

pub async fn treblle_middleware(
    State(layer): State<Arc<TreblleLayer>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let config = layer.config.clone();
    let treblle_client = layer.treblle_client.clone();
    let start_time = Instant::now();

    let should_process = !config.core.should_ignore_route(req.uri().path())
        && req
        .headers()
        .get("Content-Type")
        .and_then(|ct| ct.to_str().ok())
        .map(|ct| ct.starts_with("application/json"))
        .unwrap_or(false);

    // Process request for Treblle
    let req = if should_process {
        let (parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(body, MAX_BODY_SIZE)
            .await
            .unwrap_or_default();

        // Store original body for Treblle processing
        let mut new_req = Request::from_parts(parts, Body::from(bytes.clone()));
        new_req.extensions_mut().insert(bytes);
        new_req
    } else {
        req
    };

    if should_process {
        debug!("Processing request for Treblle: {}", req.uri().path());
        let request_payload =
            PayloadBuilder::build_request_payload::<AxumExtractor>(&req, &config.core);

        let treblle_client_clone = treblle_client.clone();
        tokio::spawn(async move {
            if let Err(e) = treblle_client_clone.send_to_treblle(request_payload).await {
                error!("Failed to send request payload to Treblle: {:?}", e);
            }
        });
    }

    let mut response = next.run(req).await;

    if should_process {
        let duration = start_time.elapsed();

        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, MAX_BODY_SIZE)
            .await
            .unwrap_or_default();

        // Store original body for Treblle processing
        response = Response::from_parts(parts, Body::from(bytes.clone()));
        response.extensions_mut().insert(bytes);

        debug!("Processing response for Treblle: {}", response.status());
        let response_payload = PayloadBuilder::build_response_payload::<AxumExtractor>(
            &response,
            &config.core,
            duration,
        );

        tokio::spawn(async move {
            if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                error!("Failed to send response payload to Treblle: {:?}", e);
            }
        });
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        extract::Json,
        http::{Request, StatusCode},
        routing::{get, post},
        Router,
    };
    use http::{header::CONTENT_TYPE, Method};
    use serde_json::{json, Value};
    use std::time::Duration;
    use tower::{ServiceBuilder, ServiceExt};
    use tower_http::timeout::TimeoutLayer;

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
        config
            .core
            .add_masked_fields(vec![
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

        let request = Request::builder()
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

        let request = Request::builder()
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

        let request = Request::builder()
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
        config
            .core
            .add_masked_fields(vec!["password".to_string()]);

        let test_data = json!({
            "username": "test_user",
            "password": "secret123"
        });

        // Create a request with sensitive data
        let mut req = Request::new(Body::empty());
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
            let req = Request::builder()
                .uri("/echo")
                .method(Method::POST)
                .header(CONTENT_TYPE, "application/json")
                .extension(bytes::Bytes::from(payload_bytes.clone()))
                .body(Body::empty())
                .unwrap();

            // Create config with all fields that should be masked
            let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
            config
                .core
                .add_masked_fields(vec![
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
            let response_req = Request::builder()
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
        let req = Request::builder()
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
        let response_req = Request::builder()
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
