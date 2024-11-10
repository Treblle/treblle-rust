#[cfg(test)]
pub mod tests {
    use std::time::Duration;
    use crate::extractors::ActixExtractor;
    use crate::{ActixConfig, Treblle, TreblleConfig, TreblleMiddleware};
    use actix_http::{header, HttpMessage, StatusCode};
    use actix_web::{http::header::ContentType, test, web, App, FromRequest, HttpResponse};
    use actix_web::dev::ServiceResponse;
    use bytes::Bytes;
    use serde_json::{json, Value};
    use treblle_core::payload::HttpExtractor;
    use treblle_core::PayloadBuilder;

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
        assert!(treblle.config.core.masked_fields.iter()
            .any(|r| r.as_str().contains("custom_field")));
        assert!(treblle.config.core.ignored_routes.iter()
            .any(|r| r.as_str().contains("/health")));
    }

    #[actix_web::test]
    async fn test_treblle_config_extraction() {
        let config = ActixConfig::new("test_key".to_string(), "test_project".to_string());

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config.clone()))
                .route("/test", web::get().to(|| async { "ok" }))
        ).await;

        let req = test::TestRequest::default()
            .to_request();

        let srv_req = test::call_service(&app, req).await;
        let config_extracted = TreblleConfig::extract(&srv_req.request()).await.unwrap();

        assert_eq!(config_extracted.0.core.api_key, config.core.api_key);
        assert_eq!(config_extracted.0.core.project_id, config.core.project_id);
    }

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
            config.core.add_masked_fields(vec![
                "number".to_string(),
                "cvv".to_string()
            ]);

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

        config.core.add_masked_fields_regex(vec![
            r"(?i)number$".to_string(),
            r"(?i)^cvv$".to_string()
        ]).unwrap();

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
            .add_masked_fields(vec!["password".to_string()]);

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
        config.core.add_ignored_routes(vec!["/ignored.*".to_string()]);
        config.core.add_masked_fields(vec!["password".to_string()]);

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