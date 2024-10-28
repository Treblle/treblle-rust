use crate::config::ActixConfig;
use crate::extractors::ActixExtractor;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web::Bytes,
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    sync::Arc,
    time::Instant,
};
use tracing::{debug, error};
use treblle_core::{PayloadBuilder, TreblleClient};

#[derive(Clone)]
pub struct TreblleMiddleware {
    config: Arc<ActixConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleMiddleware {
    pub fn new(config: ActixConfig) -> Self {
        TreblleMiddleware {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone()).expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

impl<S> Transform<S, ServiceRequest> for TreblleMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Transform = TreblleMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TreblleMiddlewareService {
            service,
            config: self.config.clone(),
            treblle_client: self.treblle_client.clone(),
        }))
    }
}

pub struct TreblleMiddlewareService<S> {
    service: S,
    config: Arc<ActixConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl<S> Service<ServiceRequest> for TreblleMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();
        let start_time = Instant::now();

        let should_process = !config
            .core
            .ignored_routes
            .iter()
            .any(|route| route.is_match(req.path()))
            && req
                .headers()
                .get("Content-Type")
                .and_then(|ct| ct.to_str().ok())
                .map(|ct| ct.starts_with("application/json"))
                .unwrap_or(false);

        if should_process {
            // For now, we'll just store a marker in extensions
            // The actual body will be handled in the extractor
            req.request().extensions_mut().insert(Bytes::new());

            debug!("Processing request for Treblle: {}", req.path());
            let request_payload =
                PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

            let treblle_client_clone = treblle_client.clone();
            actix_web::rt::spawn(async move {
                if let Err(e) = treblle_client_clone.send_to_treblle(request_payload).await {
                    error!("Failed to send request payload to Treblle: {:?}", e);
                }
            });
        }

        let fut = self.service.call(req);
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();

        Box::pin(async move {
            let res = fut.await?;

            if should_process {
                let duration = start_time.elapsed();

                // Store a marker in extensions
                res.request().extensions_mut().insert(Bytes::new());

                debug!("Processing response for Treblle: {}", res.status());
                let response_payload = PayloadBuilder::build_response_payload::<ActixExtractor>(
                    &res,
                    &config.core,
                    duration,
                );

                actix_web::rt::spawn(async move {
                    if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                        error!("Failed to send response payload to Treblle: {:?}", e);
                    }
                });
            }

            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_http::StatusCode;
    use actix_web::{http::header::ContentType, test, web, App, HttpResponse};
    use bytes::Bytes;
    use serde_json::{json, Value};

    async fn echo_handler(body: web::Json<Value>) -> HttpResponse {
        HttpResponse::Ok().json(body.0)
    }

    async fn text_handler() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("text/plain")
            .body("Hello, World!")
    }

    async fn ignored_handler(body: web::Json<Value>) -> HttpResponse {
        HttpResponse::Ok().json(body.0)
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

    fn setup_test_config() -> ActixConfig {
        let mut config = ActixConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec![
                "password".to_string(),
                "Password".to_string(),
                "user_password".to_string(),
                "credit_card".to_string(),
                "cvv".to_string(),
                "ssn".to_string(),
            ])
            .expect("Failed to add masked fields");
        config
    }

    #[actix_web::test]
    async fn test_data_masking_patterns() {
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
        ];

        for (payload, fields_to_check) in test_cases {
            let config = setup_test_config();
            let app = test::init_service(
                App::new()
                    .wrap(TreblleMiddleware::new(config.clone()))
                    .route("/echo", web::post().to(echo_handler)),
            )
            .await;

            let payload_bytes = payload.to_string();

            // Create the service request
            let req = test::TestRequest::post()
                .uri("/echo")
                .insert_header(("content-type", "application/json"))
                .set_payload(payload_bytes.clone())
                .to_srv_request();

            // Add the body to the request extensions for the middleware to process
            req.extensions_mut()
                .insert(Bytes::from(payload_bytes.clone()));

            let treblle_payload =
                PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

            // Verify masking in Treblle payload
            if let Some(body) = treblle_payload.data.request.body {
                for &field in &fields_to_check {
                    if let Some(value) = find_field_value(&body, field) {
                        assert_eq!(value, "*****", "Field '{}' was not properly masked", field);
                    }
                }
            }

            // Test that original response is unmasked
            let app_req = test::TestRequest::post()
                .uri("/echo")
                .insert_header(("content-type", "application/json"))
                .set_payload(payload_bytes.clone())
                .to_request();

            let resp = test::call_service(&app, app_req).await;
            assert!(resp.status().is_success());

            let body: Value = test::read_body_json(resp).await;

            // Verify original data is preserved
            for &field in &fields_to_check {
                if let Some(original_value) = find_field_value(&payload, field) {
                    if let Some(response_value) = find_field_value(&body, field) {
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

    #[actix_web::test]
    async fn test_partial_field_masking() {
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

        let mut config = ActixConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec!["number".to_string(), "cvv".to_string()])
            .expect("Failed to add masked fields");

        let app = test::init_service(
            App::new()
                .wrap(TreblleMiddleware::new(config.clone()))
                .route("/echo", web::post().to(echo_handler)),
        )
        .await;

        let payload_bytes = test_data.to_string();

        // Create service request for testing Treblle payload
        let req = test::TestRequest::post()
            .uri("/echo")
            .insert_header(("content-type", "application/json"))
            .set_payload(payload_bytes.clone())
            .to_srv_request();

        // Add body to request extensions for middleware processing
        req.extensions_mut()
            .insert(Bytes::from(payload_bytes.clone()));

        let treblle_payload =
            PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

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

        // Create request for testing response
        let app_req = test::TestRequest::post()
            .uri("/echo")
            .insert_header(("content-type", "application/json"))
            .set_payload(payload_bytes.clone())
            .to_request();

        // Test that original response is unmasked
        let resp = test::call_service(&app, app_req).await;
        assert!(resp.status().is_success());

        let body: Value = test::read_body_json(resp).await;

        // Verify original data is preserved
        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["credit_card"]["number"], "4111-1111-1111-1111");
        assert_eq!(body["user"]["credit_card"]["expiry"], "12/24");
        assert_eq!(body["user"]["credit_card"]["cvv"], "123");
        assert_eq!(body["user"]["shipping_address"]["street"], "123 Main St");
    }

    #[actix_web::test]
    async fn test_treblle_payload_creation() {
        let mut config = ActixConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec!["password".to_string()])
            .unwrap();

        let _app = test::init_service(
            App::new()
                .wrap(TreblleMiddleware::new(config.clone()))
                .route("/echo", web::post().to(echo_handler)),
        )
        .await;

        let test_data = json!({
            "username": "test_user",
            "password": "secret123"
        });

        let payload_bytes = test_data.to_string();

        // Create service request for testing Treblle payload
        let req = test::TestRequest::post()
            .uri("/echo")
            .insert_header(ContentType::json())
            .set_payload(payload_bytes.clone())
            .to_srv_request();

        req.extensions_mut().insert(Bytes::from(payload_bytes));

        let treblle_payload =
            PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

        if let Some(body) = &treblle_payload.data.request.body {
            assert_eq!(body["username"].as_str().unwrap(), "test_user");
            assert_eq!(body["password"].as_str().unwrap(), "*****");
        }
    }

    #[actix_web::test]
    async fn test_middleware_allows_non_json_requests() {
        let config = setup_test_config();
        let app = test::init_service(
            App::new()
                .wrap(TreblleMiddleware::new(config))
                .route("/text", web::get().to(text_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/text").to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp).await;
        assert_eq!(body, Bytes::from("Hello, World!"));
    }

    #[actix_web::test]
    async fn test_middleware_respects_ignored_routes() {
        let mut config = ActixConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_ignored_routes(vec!["/ignored.*".to_string()])
            .unwrap();
        config
            .core
            .add_masked_fields(vec!["password".to_string()])
            .unwrap();

        let app = test::init_service(
            App::new()
                .wrap(TreblleMiddleware::new(config))
                .route("/ignored", web::post().to(ignored_handler)),
        )
        .await;

        let test_data = json!({
            "password": "secret123",
            "credit_card": "4111-1111-1111-1111"
        });

        let req = test::TestRequest::post()
            .uri("/ignored")
            .insert_header(ContentType::json())
            .set_json(test_data)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["password"], "secret123");
        assert_eq!(body["credit_card"], "4111-1111-1111-1111");
    }

    #[actix_web::test]
    async fn test_middleware_preserves_original_data() {
        let config = setup_test_config();
        let app = test::init_service(
            App::new()
                .wrap(TreblleMiddleware::new(config))
                .route("/echo", web::post().to(echo_handler)),
        )
        .await;

        let test_data = json!({
            "user": {
                "email": "test@example.com",
                "password": "secret123",
                "credit_card": "4111-1111-1111-1111"
            }
        });

        let req = test::TestRequest::post()
            .uri("/echo")
            .insert_header(ContentType::json())
            .set_json(test_data)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: Value = test::read_body_json(resp).await;
        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["password"], "secret123");
        assert_eq!(body["user"]["credit_card"], "4111-1111-1111-1111");
    }
}
