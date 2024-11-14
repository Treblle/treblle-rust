use std::sync::Arc;

use actix_web::{
    test,
    web::{self, Bytes},
    App, HttpResponse,
};
use serde_json::{json, Value};
use treblle_actix::{ActixConfig, TreblleMiddleware};

// Test handler
async fn echo_handler(json: web::Json<Value>) -> HttpResponse {
    HttpResponse::Ok().json(json.into_inner())
}

// Test helper to create app with Treblle middleware
fn create_test_app() -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .add_masked_fields(vec!["password", "credit_card"])
        .add_ignored_routes(vec!["/health"])
        .build()
        .unwrap();

    App::new().wrap(TreblleMiddleware::new(config)).route("/test", web::post().to(echo_handler))
}

fn generate_deeply_nested_json(depth: usize) -> Value {
    let mut value = json!({"data": "leaf"});
    for _ in 0..depth {
        value = json!({ "nested": value });
    }
    value
}

#[actix_web::test]
async fn test_various_payload_types() {
    let app = test::init_service(create_test_app()).await;

    // Test different JSON payload types
    let test_cases = vec![
        json!(null),
        json!(true),
        json!(42),
        json!("string"),
        json!(["array", "of", "values"]),
        json!({
            "nested": {
                "object": {
                    "with": "values"
                }
            }
        }),
        json!({
            "mixed": {
                "array": [1, "two", false],
                "number": 42,
                "string": "value"
            }
        }),
    ];

    for payload in test_cases {
        let req = test::TestRequest::post().uri("/test").set_json(&payload).to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: Bytes = test::read_body(resp).await;
        let response_json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(response_json, payload);
    }
}

#[actix_web::test]
async fn test_concurrent_requests() {
    let app = Arc::new(test::init_service(create_test_app()).await);
    let mut handles = Vec::new();

    // Launch 100 concurrent requests
    for i in 0..100 {
        let app = Arc::clone(&app);
        let handle = actix_web::rt::spawn(async move {
            let payload = json!({
                "request_id": i,
                "test": "data",
                "password": "secret",
                "nested": {
                    "credit_card": "4111-1111-1111-1111"
                }
            });

            let req = test::TestRequest::post().uri("/test").set_json(&payload).to_request();

            let resp = test::call_service(app.as_ref(), req).await;
            assert!(resp.status().is_success());

            let body: Bytes = test::read_body(resp).await;
            let response_json: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(response_json, payload);
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[actix_web::test]
async fn test_edge_cases() {
    let app = test::init_service(create_test_app()).await;

    // Test empty object
    let req = test::TestRequest::post().uri("/test").set_json(json!({})).to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Test large nested payload
    let mut large_obj = json!({});
    for i in 0..100 {
        large_obj[format!("key_{}", i)] = json!({
            "nested": {
                "data": format!("value_{}", i),
                "array": vec![1, 2, 3, 4, 5]
            }
        });
    }

    let req = test::TestRequest::post().uri("/test").set_json(large_obj.clone()).to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: Bytes = test::read_body(resp).await;
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json, large_obj);
}

#[actix_web::test]
async fn test_sensitive_data_masking() {
    let app = test::init_service(create_test_app()).await;

    let payload = json!({
        "username": "test_user",
        "password": "super_secret",
        "data": {
            "credit_card": "4111-1111-1111-1111",
            "public_info": "visible"
        }
    });

    let req = test::TestRequest::post().uri("/test").set_json(&payload).to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: Bytes = test::read_body(resp).await;
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json, payload);
}

#[actix_web::test]
async fn test_malformed_payloads() {
    let app = test::init_service(create_test_app()).await;

    // Test malformed/unexpected JSON payloads
    let fuzz_cases = vec![
        // Deeply nested objects (potential stack overflow)
        generate_deeply_nested_json(100),
        // Very large strings
        json!({ "large_string": "x".repeat(1_000_000) }),
        // Arrays with mixed types
        json!([1, "string", true, null, {"key": [1,2,3]}, [[[]]]] ),
        // Unicode edge cases
        json!({
            "unicode": "🦀💻🔥\u{0000}\u{FFFF}",
            "rtl": "שָׁלוֹם עֲלֵיכֶם",
            "special": "\n\r\t"  // Simplified special characters
        }),
        // Numbers edge cases
        json!({
            "max_int": i64::MAX,
            "min_int": i64::MIN,
            "float": f64::MAX,
            "neg_float": f64::MIN,
            "infinity": f64::INFINITY
        }),
    ];

    for payload in fuzz_cases {
        let req = test::TestRequest::post().uri("/test").set_json(&payload).to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success() || resp.status().as_u16() == 400);
    }
}

#[actix_web::test]
async fn test_error_cases() {
    let app = test::init_service(create_test_app()).await;

    // Test various error scenarios
    let error_cases = vec![
        // Invalid JSON
        (r#"{"unclosed": "object"#, "Invalid JSON syntax"),
        (r#"["unclosed array"#, "Invalid JSON syntax"),
        // Invalid Content-Type
        (r#"{"valid": "json"}"#, "Wrong content type"),
        // Empty body
        ("", "Empty request body"),
        // Non-JSON data
        ("<html>not json</html>", "Invalid JSON content"),
    ];

    for (payload, error_desc) in error_cases {
        let mut req = test::TestRequest::post().uri("/test").set_payload(payload.to_string());

        if error_desc != "Wrong content type" {
            req = req.insert_header(("content-type", "application/json"));
        }

        let resp = test::call_service(&app, req.to_request()).await;
        assert!(resp.status().is_client_error(), "Failed for case: {}", error_desc);
    }
}

#[actix_web::test]
async fn test_headers_and_methods() {
    let app = test::init_service(create_test_app()).await;

    // Test various HTTP methods and headers
    let test_cases = vec![
        // Different HTTP methods with expected status codes
        (test::TestRequest::get().uri("/test"), 404), // Method not allowed
        (test::TestRequest::put().uri("/test"), 404), // Method not allowed
        (test::TestRequest::delete().uri("/test"), 404), // Method not allowed
        // POST requests with various headers should succeed
        (test::TestRequest::post().uri("/test").insert_header(("X-Custom-Header", "value")), 200),
        (test::TestRequest::post().uri("/test").insert_header(("User-Agent", "Custom/1.0")), 200),
        (test::TestRequest::post().uri("/test").insert_header(("Accept", "application/json")), 200),
    ];

    for (req, expected_status) in test_cases {
        let resp =
            test::call_service(&app, req.set_json(json!({"test": "data"})).to_request()).await;

        assert_eq!(
            resp.status().as_u16(),
            expected_status,
            "Expected status {} but got {} for request",
            expected_status,
            resp.status().as_u16()
        );
    }
}

#[actix_web::test]
async fn test_performance() {
    let app = test::init_service(create_test_app()).await;
    let start = std::time::Instant::now();
    let iterations = 1000;

    for i in 0..iterations {
        let payload = json!({
            "index": i,
            "nested": {
                "data": "some value",
                "array": [1, 2, 3],
                "object": {"key": "value"}
            }
        });

        let req = test::TestRequest::post().uri("/test").set_json(&payload).to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    let duration = start.elapsed();
    let avg_response_time = duration.as_micros() as f64 / iterations as f64;

    // Assert reasonable performance (adjust threshold as needed)
    assert!(
        avg_response_time < 1000.0, // 1ms
        "Average response time too high: {:.2}µs",
        avg_response_time
    );
}
