use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use treblle_axum::{AxumConfig, Treblle, TreblleExt};

async fn echo_handler(Json(json): Json<Value>) -> Json<Value> {
    Json(json)
}

fn create_test_app() -> Router {
    let config = AxumConfig::builder()
        .api_key("test_key")
        .add_masked_fields(vec!["password", "credit_card"])
        .add_ignored_routes(vec!["/health"])
        .build()
        .unwrap();

    let treblle = Treblle::from_config(config);

    Router::new().route("/test", post(echo_handler)).treblle(treblle)
}

fn generate_deeply_nested_json(depth: usize) -> Value {
    let mut value = json!({"data": "leaf"});
    for _ in 0..depth {
        value = json!({ "nested": value });
    }
    value
}

#[tokio::test]
async fn test_various_payload_types() {
    let app = create_test_app();

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
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(response_json, payload);
    }
}

#[tokio::test]
async fn test_concurrent_requests() {
    let app = create_test_app();
    let mut handles = Vec::new();

    for i in 0..100 {
        let app = app.clone();
        let handle = tokio::spawn(async move {
            let payload = json!({
                "request_id": i,
                "test": "data",
                "password": "secret",
                "nested": {
                    "credit_card": "4111-1111-1111-1111"
                }
            });

            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/test")
                        .header("content-type", "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let response_json: Value = serde_json::from_slice(&body_bytes).unwrap();
            assert_eq!(response_json, payload);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_edge_cases() {
    let app = create_test_app();

    // Test empty object
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test large nested payload
    let mut large_obj = json!({});
    for i in 0..100 {
        large_obj[format!("key_{i}")] = json!({
            "nested": {
                "data": format!("value_{}", i),
                "array": vec![1, 2, 3, 4, 5]
            }
        });
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(large_obj.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(response_json, large_obj);
}

#[tokio::test]
async fn test_sensitive_data_masking() {
    let app = create_test_app();

    let payload = json!({
        "username": "test_user",
        "password": "super_secret",
        "data": {
            "credit_card": "4111-1111-1111-1111",
            "public_info": "visible"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(response_json, payload);
}

#[tokio::test]
async fn test_malformed_payloads() {
    let app = create_test_app();

    let fuzz_cases = vec![
        generate_deeply_nested_json(100),
        json!({ "large_string": "x".repeat(1_000_000) }),
        json!([1, "string", true, null, {"key": [1,2,3]}, [[[]]]]),
        json!({
            "unicode": "🦀💻🔥\u{0000}\u{FFFF}",
            "rtl": "שָׁלוֹם עֲלֵיכֶם",
            "special": "\n\r\t"
        }),
        json!({
            "max_int": i64::MAX,
            "min_int": i64::MIN,
            "float": f64::MAX,
            "neg_float": f64::MIN,
            "infinity": f64::INFINITY
        }),
    ];

    for payload in fuzz_cases {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_success() || response.status() == StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn test_error_cases() {
    let app = create_test_app();

    let error_cases = vec![
        (r#"{"unclosed": "object"#, "Invalid JSON syntax", true),
        (r#"["unclosed array"#, "Invalid JSON syntax", true),
        (r#"{"valid": "json"}"#, "Wrong content type", false),
        ("", "Empty request body", true),
        ("<html>not json</html>", "Invalid JSON content", true),
    ];

    for (payload, error_desc, include_content_type) in error_cases {
        let mut req = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(payload.to_string()))
            .unwrap();

        if include_content_type {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
        }

        let response = app.clone().oneshot(req).await.unwrap();
        assert!(response.status().is_client_error(), "Failed for case: {error_desc}");
    }
}

#[tokio::test]
async fn test_headers_and_methods() {
    let app = create_test_app();

    let test_cases = vec![
        ("GET", "/test", StatusCode::METHOD_NOT_ALLOWED),
        ("PUT", "/test", StatusCode::METHOD_NOT_ALLOWED),
        ("DELETE", "/test", StatusCode::METHOD_NOT_ALLOWED),
        ("POST", "/test", StatusCode::OK),
    ];

    for (method, uri, expected_status) in test_cases {
        let mut req =
            Request::builder().method(method).uri(uri).header("content-type", "application/json");

        if method == "POST" {
            req = req
                .header("X-Custom-Header", "value")
                .header("User-Agent", "Custom/1.0")
                .header("Accept", "application/json");
        }

        let response = app
            .clone()
            .oneshot(req.body(Body::from(json!({"test": "data"}).to_string())).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            expected_status,
            "Expected status {:?} but got {:?} for {} request",
            expected_status,
            response.status(),
            method
        );
    }
}

#[tokio::test]
async fn test_performance() {
    let app = create_test_app();
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

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    let duration = start.elapsed();
    let avg_response_time = duration.as_micros() as f64 / f64::from(iterations);

    assert!(avg_response_time < 1000.0, "Average response time too high: {avg_response_time:.2}µs");
}
