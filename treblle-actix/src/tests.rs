#[cfg(test)]
pub mod tests {
    use crate::extractors::ActixExtractor;
    use crate::{ActixConfig, Treblle, TreblleConfig, TreblleMiddleware};
    use actix_http::{header, HttpMessage, StatusCode};
    use actix_web::dev::ServiceResponse;
    use actix_web::{test, web, App, FromRequest, HttpResponse};
    use bytes::Bytes;
    use futures_util::FutureExt;
    use serde_json::{json, Value};
    use std::time::Duration;
    use treblle_core::extractors::TreblleExtractor;
    use treblle_core::PayloadBuilder;

    mod test_utils {
        use super::*;

        pub fn create_test_request(headers: Vec<(&str, &str)>) -> actix_web::dev::ServiceRequest {
            let _app = test::init_service(
                App::new().default_service(web::to(|| async { HttpResponse::Ok().finish() })),
            )
            .now_or_never()
            .unwrap();

            let mut req = test::TestRequest::default().uri("/test");

            for (name, value) in headers {
                req = req.insert_header((name, value));
            }

            req.to_srv_request()
        }

        // Helper function to recursively find a field value in a JSON object
        pub fn find_field_value<'a>(json: &'a Value, field: &str) -> Option<&'a str> {
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

        pub fn setup_test_config() -> ActixConfig {
            let mut config = ActixConfig::new("test_key".to_string(), "test_project".to_string());
            config.core.add_masked_fields(vec![
                "password".to_string(),
                "Password".to_string(),
                "user_password".to_string(),
                "credit_card".to_string(),
                "cvv".to_string(),
                "ssn".to_string(),
            ]);
            config
        }

        pub async fn echo_handler(body: web::Json<Value>) -> HttpResponse {
            HttpResponse::Ok().json(body.0)
        }

        pub async fn text_handler() -> HttpResponse {
            HttpResponse::Ok().content_type("text/plain").body("Hello, World!")
        }

        pub async fn ignored_handler(body: web::Json<Value>) -> HttpResponse {
            HttpResponse::Ok().json(body.0)
        }
    }

    mod server_tests {
        use super::*;

        #[actix_web::test]
        async fn test_server_info() {
            let server_info = ActixExtractor::extract_server_info();

            assert!(!server_info.ip.is_empty());
            assert!(!server_info.timezone.is_empty());
            assert!(server_info.software.as_ref().unwrap().contains("actix-web"));
            assert_eq!(server_info.protocol, "HTTP/1.1");
            assert!(!server_info.os.name.is_empty());
            assert!(!server_info.os.release.is_empty());
            assert!(!server_info.os.architecture.is_empty());

            // Test caching - should return the same instance
            let server_info2 = ActixExtractor::extract_server_info();
            assert_eq!(format!("{:?}", server_info), format!("{:?}", server_info2));
        }
    }

    mod request_tests {
        use super::*;
        use test_utils::create_test_request;

        #[actix_web::test]
        async fn test_url_construction() {
            let test_cases = vec![
                (
                    vec![("Host", "api.example.com"), ("X-Forwarded-Proto", "https")],
                    "/test",
                    "https://api.example.com/test",
                ),
                (
                    vec![("Host", "localhost:8080")],
                    "/api/v1/users",
                    "http://localhost:8080/api/v1/users",
                ),
                (
                    vec![("Host", "api.example.com"), ("X-Forwarded-Proto", "https")],
                    "/test?query=value",
                    "https://api.example.com/test?query=value",
                ),
            ];

            for (headers, path, expected_url) in test_cases {
                let _app = test::init_service(
                    App::new().default_service(web::to(|| async { HttpResponse::Ok().finish() })),
                )
                .await;

                let mut req = test::TestRequest::default().uri(path);
                for (name, value) in headers {
                    req = req.insert_header((name, value));
                }

                let srv_req = req.to_srv_request();
                let info = ActixExtractor::extract_request_info(&srv_req);
                assert_eq!(info.url, expected_url);
            }
        }

        #[actix_web::test]
        async fn test_ip_extraction() {
            let test_cases = vec![
                (vec![("X-Forwarded-For", "203.0.113.195")], "203.0.113.195"),
                (vec![("X-Real-IP", "203.0.113.196")], "203.0.113.196"),
                (vec![("Forwarded", "for=192.0.2.60;proto=http;by=203.0.113.43")], "192.0.2.60"),
                (
                    vec![("X-Forwarded-For", "203.0.113.195"), ("X-Real-IP", "203.0.113.196")],
                    "203.0.113.195", // X-Forwarded-For takes precedence
                ),
                (
                    vec![("Forwarded", "for=192.0.2.60"), ("X-Forwarded-For", "203.0.113.195")],
                    "192.0.2.60", // Forwarded header takes precedence
                ),
            ];

            for (headers, expected_ip) in test_cases {
                let req = create_test_request(headers);
                let info = ActixExtractor::extract_request_info(&req);
                assert_eq!(info.ip, expected_ip);
            }
        }

        #[actix_web::test]
        async fn test_extract_request_info() {
            let req =
                create_test_request(vec![("User-Agent", "test-agent"), ("Host", "localhost:8080")]);

            let info = ActixExtractor::extract_request_info(&req);

            assert_eq!(info.url, "http://localhost:8080/test");
            assert_eq!(info.method, "GET");
            assert_eq!(info.user_agent, "test-agent");
        }
    }

    mod response_tests {
        use super::*;

        #[actix_web::test]
        async fn test_empty_body_handling() {
            let res = HttpResponse::Ok().content_type("application/json").body(Bytes::new());

            let resp = ServiceResponse::new(test::TestRequest::default().to_http_request(), res);

            let info = ActixExtractor::extract_response_info(&resp, Duration::from_secs(1));
            assert!(info.body.is_none());
        }

        #[actix_web::test]
        async fn test_invalid_json_body() {
            let req = test::TestRequest::default().to_http_request();
            req.extensions_mut().insert(Bytes::from("invalid json"));

            let resp = ServiceResponse::new(
                req,
                HttpResponse::BadRequest().content_type("application/json").body("invalid json"),
            );

            let info = ActixExtractor::extract_response_info(&resp, Duration::from_secs(1));
            assert!(info.body.is_none());
        }

        #[actix_web::test]
        async fn test_extract_response_info() {
            let json_body = json!({"result": "success"});
            let req = test::TestRequest::default().to_http_request();
            req.extensions_mut().insert(Bytes::from(json_body.to_string()));

            let resp = ServiceResponse::new(
                req,
                HttpResponse::Ok().content_type("application/json").body(json_body.to_string()),
            );

            let info = ActixExtractor::extract_response_info(&resp, Duration::from_secs(1));

            assert_eq!(info.code, 200);
            assert_eq!(info.load_time, 1.0);
            assert!(info.body.is_some());
        }

        #[actix_web::test]
        async fn test_response_size_calculation() {
            let test_cases = vec![
                (
                    json!({
                        "key": "value",
                        "nested": {
                            "array": [1, 2, 3]
                        }
                    }),
                    None, // No explicit Content-Length
                ),
                (json!("simple string"), Some(15)),
                (json!({"empty": {}}), Some(12)),
            ];

            for (body, expected_size) in test_cases {
                let body_string = body.to_string();
                let body_len = body_string.len() as u64;

                let mut req_builder = test::TestRequest::default();

                if let Some(size) = expected_size {
                    req_builder = req_builder.insert_header((
                        header::CONTENT_LENGTH,
                        header::HeaderValue::from_str(&size.to_string()).unwrap(),
                    ));
                }

                let req = req_builder.to_http_request();

                let resp = ServiceResponse::new(
                    req,
                    HttpResponse::Ok().content_type("application/json").body(body_string.clone()),
                );

                let info = ActixExtractor::extract_response_info(&resp, Duration::from_secs(1));

                let expected = expected_size.unwrap_or(body_len);
                assert_eq!(
                    info.size,
                    expected,
                    "Size mismatch for body: {}. Expected: {}, got: {}. Body length: {}",
                    body,
                    expected,
                    info.size,
                    body_string.len()
                );
            }
        }
    }

    mod error_tests {
        use super::*;

        #[actix_web::test]
        async fn test_extract_error_info() {
            let error_body = json!({
                "error": "Not Found",
                "message": "Resource does not exist"
            });

            let req = test::TestRequest::default().to_http_request();
            req.extensions_mut().insert(Bytes::from(error_body.to_string()));

            let resp = ServiceResponse::new(
                req,
                HttpResponse::NotFound()
                    .content_type("application/json")
                    .body(error_body.to_string()),
            );

            let errors = ActixExtractor::extract_error_info(&resp).unwrap();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].error_type, "HTTP_404");
            assert!(errors[0].message.contains("Resource does not exist"));
        }

        #[actix_web::test]
        async fn test_error_message_handling() {
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
                (StatusCode::BAD_REQUEST, json!({"custom": "format"}), "{\"custom\":\"format\"}"),
            ];

            for (status, error_body, expected_message) in test_cases {
                let req = test::TestRequest::default().to_http_request();
                req.extensions_mut().insert(Bytes::from(error_body.to_string()));

                let resp = ServiceResponse::new(
                    req,
                    HttpResponse::build(status)
                        .content_type("application/json")
                        .body(error_body.to_string()),
                );

                let errors = ActixExtractor::extract_error_info(&resp).unwrap();
                assert_eq!(errors[0].source, "actix");
                assert_eq!(errors[0].error_type, format!("HTTP_{}", status.as_u16()));
                assert_eq!(errors[0].message, expected_message);
            }
        }
    }

    mod config_tests {
        use super::*;

        #[actix_web::test]
        async fn test_actix_config() {
            let config = ActixConfig::new("test_key".to_string(), "test_project".to_string())
                .buffer_response(true);
            assert_eq!(config.core.api_key, "test_key");
            assert_eq!(config.core.project_id, "test_project");
            assert!(config.buffer_response);
        }

        #[actix_web::test]
        async fn test_treblle_builder() {
            let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
                .add_masked_fields(vec!["custom_field".to_string()])
                .add_ignored_routes(vec!["/health".to_string()]);

            assert_eq!(treblle.config.core.api_key, "api_key");
            assert_eq!(treblle.config.core.project_id, "project_id");
            assert!(treblle
                .config
                .core
                .masked_fields
                .iter()
                .any(|r| r.as_str().contains("custom_field")));
            assert!(treblle
                .config
                .core
                .ignored_routes
                .iter()
                .any(|r| r.as_str().contains("/health")));
        }

        #[actix_web::test]
        async fn test_treblle_config_extraction() {
            let config = ActixConfig::new("test_key".to_string(), "test_project".to_string());

            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(config.clone()))
                    .route("/test", web::get().to(|| async { "ok" })),
            )
            .await;

            let req = test::TestRequest::default().to_request();

            let srv_req = test::call_service(&app, req).await;
            let config_extracted = TreblleConfig::extract(&srv_req.request()).await.unwrap();

            assert_eq!(config_extracted.0.core.api_key, config.core.api_key);
            assert_eq!(config_extracted.0.core.project_id, config.core.project_id);
        }
    }

    mod integration_tests {
        use super::*;
        use test_utils::{find_field_value, setup_test_config};

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
                let mut config = setup_test_config();
                config.core.add_masked_fields(vec!["number".to_string(), "cvv".to_string()]);

                let app = test::init_service(
                    App::new()
                        .wrap(TreblleMiddleware::new(config.clone()))
                        .route("/echo", web::post().to(test_utils::echo_handler)),
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
                req.extensions_mut().insert(Bytes::from(payload_bytes.clone()));

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
                .add_masked_fields_regex(vec![r"(?i)number$".to_string(), r"(?i)^cvv$".to_string()])
                .unwrap();

            let app = test::init_service(
                App::new()
                    .wrap(TreblleMiddleware::new(config.clone()))
                    .route("/echo", web::post().to(test_utils::echo_handler)),
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
            req.extensions_mut().insert(Bytes::from(payload_bytes.clone()));

            let treblle_payload =
                PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

            // Verify masking in Treblle payload
            if let Some(body) = treblle_payload.data.request.body {
                assert_eq!(body["user"]["email"].as_str().unwrap(), "test@example.com");
                assert_eq!(body["user"]["credit_card"]["number"].as_str().unwrap(), "*****");
                assert_eq!(body["user"]["credit_card"]["expiry"].as_str().unwrap(), "12/24");
                assert_eq!(body["user"]["credit_card"]["cvv"].as_str().unwrap(), "*****");
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
        async fn test_middleware_allows_non_json_requests() {
            let app = test::init_service(
                App::new()
                    .wrap(TreblleMiddleware::new(setup_test_config()))
                    .route("/text", web::get().to(test_utils::text_handler)),
            )
            .await;

            let req = test::TestRequest::get()
                .uri("/text")
                .insert_header(("Accept", "text/plain"))
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success());

            let body = test::read_body(resp).await;
            assert_eq!(body, Bytes::from("Hello, World!"));
        }

        #[actix_web::test]
        async fn test_middleware_preserves_original_data() {
            let app = test::init_service(
                App::new()
                    .wrap(TreblleMiddleware::new(setup_test_config()))
                    .route("/echo", web::post().to(test_utils::echo_handler)),
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
                .insert_header(("content-type", "application/json"))
                .set_payload(test_data.to_string())
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success());

            let body: Value = test::read_body_json(resp).await;
            assert_eq!(body["user"]["email"], "test@example.com");
            assert_eq!(body["user"]["password"], "secret123");
            assert_eq!(body["user"]["credit_card"], "4111-1111-1111-1111");
        }

        #[actix_web::test]
        async fn test_middleware_respects_ignored_routes() {
            let mut config = setup_test_config();
            config.core.add_ignored_routes(vec!["/ignored".to_string()]);

            let app = test::init_service(
                App::new()
                    .wrap(TreblleMiddleware::new(config))
                    .route("/ignored", web::post().to(test_utils::ignored_handler)),
            )
            .await;

            let test_data = json!({
                "password": "secret123",
                "credit_card": "4111-1111-1111-1111"
            });

            let req = test::TestRequest::post()
                .uri("/ignored")
                .insert_header(("content-type", "application/json"))
                .set_payload(test_data.to_string())
                .to_request();

            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success());

            let body: Value = test::read_body_json(resp).await;
            assert_eq!(body["password"], "secret123");
            assert_eq!(body["credit_card"], "4111-1111-1111-1111");
        }

        #[actix_web::test]
        async fn test_treblle_payload_creation() {
            let mut config = setup_test_config();
            config.core.add_masked_fields(vec!["password".to_string()]);

            let test_data = json!({
                "username": "test_user",
                "password": "secret123"
            });

            let req = test::TestRequest::default().to_srv_request();
            req.extensions_mut().insert(Bytes::from(test_data.to_string()));

            let treblle_payload =
                PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

            if let Some(body) = &treblle_payload.data.request.body {
                assert_eq!(body["username"], "test_user");
                assert_eq!(body["password"], "*****");
            }
        }
    }
}
