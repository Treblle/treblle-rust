use actix_http::HttpMessage;
use actix_web::{test, web, App, HttpResponse};
use bytes::Bytes;
use serde_json::{json, Value};
use treblle_actix::extractors::ActixExtractor;
use treblle_actix::{ActixConfig, TreblleMiddleware};
use treblle_core::PayloadBuilder;

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

pub async fn echo_handler(body: web::Json<Value>) -> HttpResponse {
    HttpResponse::Ok().json(body.0)
}

pub async fn text_handler() -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain").body("Hello, World!")
}

pub async fn ignored_handler(body: web::Json<Value>) -> HttpResponse {
    HttpResponse::Ok().json(body.0)
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
        let config = ActixConfig::builder()
            .api_key("test_key")
            .project_id("test_project")
            .add_masked_fields(vec!["number".to_string(), "cvv".to_string()])
            .build()
            .unwrap();

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

    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_masked_fields_regex(vec![r"(?i)number$".to_string(), r"(?i)^cvv$".to_string()])
        .unwrap()
        .build()
        .unwrap();

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
    req.extensions_mut().insert(Bytes::from(payload_bytes.clone()));

    let treblle_payload =
        PayloadBuilder::build_request_payload::<ActixExtractor>(&req, &config.core);

    // Verify masking in Treblle payload
    if let Some(body) = treblle_payload.data.request.body {
        assert_eq!(body["user"]["email"].as_str().unwrap(), "test@example.com");
        assert_eq!(body["user"]["credit_card"]["number"].as_str().unwrap(), "*****");
        assert_eq!(body["user"]["credit_card"]["expiry"].as_str().unwrap(), "12/24");
        assert_eq!(body["user"]["credit_card"]["cvv"].as_str().unwrap(), "*****");
        assert_eq!(body["user"]["shipping_address"]["street"].as_str().unwrap(), "123 Main St");
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
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_masked_fields(vec![
            "password".to_string(),
            "Password".to_string(),
            "user_password".to_string(),
            "credit_card".to_string(),
            "cvv".to_string(),
            "ssn".to_string(),
        ])
        .build()
        .unwrap();
    
    let app = test::init_service(
        App::new()
            .wrap(TreblleMiddleware::new(config))
            .route("/text", web::get().to(text_handler)),
    )
    .await;

    let req =
        test::TestRequest::get().uri("/text").insert_header(("Accept", "text/plain")).to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    assert_eq!(body, Bytes::from("Hello, World!"));
}

#[actix_web::test]
async fn test_middleware_preserves_original_data() {
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_masked_fields(vec![
            "password".to_string(),
            "Password".to_string(),
            "user_password".to_string(),
            "credit_card".to_string(),
            "cvv".to_string(),
            "ssn".to_string(),
        ])
        .build()
        .unwrap();
    
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
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_ignored_routes(vec!["/ignored".to_string()])
        .build()
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
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_masked_fields(vec!["password".to_string()])
        .build()
        .unwrap();

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
