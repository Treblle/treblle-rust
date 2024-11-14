use rocket::{
    http::{ContentType, Header, Status},
    local::blocking::Client,
    post, routes,
    serde::json::Json,
    Build, Rocket,
};
use serde_json::{json, Value};
use treblle_rocket::TreblleExt;

// Test handler
#[post("/test", format = "json", data = "<json>")]
fn echo_handler(json: Json<Value>) -> Json<Value> {
    json
}

fn create_test_rocket() -> Rocket<Build> {
    let rocket = rocket::build();
    rocket.mount("/", routes![echo_handler]).attach_treblle("test_key".to_string())
}

fn generate_deeply_nested_json(depth: usize) -> Value {
    let mut value = json!({"data": "leaf"});
    for _ in 0..depth {
        value = json!({ "nested": value });
    }
    value
}

#[test]
fn test_various_payload_types() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");

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
        let response =
            client.post("/test").header(ContentType::JSON).body(payload.to_string()).dispatch();

        assert_eq!(response.status(), Status::Ok);
        let response_json: Value = response.into_json().expect("valid JSON response");
        assert_eq!(response_json, payload);
    }
}

#[test]
fn test_malformed_payloads() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");

    let fuzz_cases = vec![
        // Deeply nested objects
        generate_deeply_nested_json(100),
        // Very large strings
        json!({ "large_string": "x".repeat(1_000_000) }),
        // Arrays with mixed types
        json!([1, "string", true, null, {"key": [1,2,3]}, [[[]]]]),
        // Unicode edge cases
        json!({
            "unicode": "🦀💻🔥\u{0000}\u{FFFF}",
            "rtl": "שָׁלוֹם עֲלֵיכֶם",
            "special": "\n\r\t"
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
        let response =
            client.post("/test").header(ContentType::JSON).body(payload.to_string()).dispatch();

        assert!(
            response.status() == Status::Ok || response.status() == Status::BadRequest,
            "Unexpected status: {}",
            response.status()
        );
    }
}

#[test]
fn test_error_cases() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");

    let error_cases = vec![
        (r#"{"unclosed": "object"#, "Invalid JSON syntax", true),
        (r#"["unclosed array"#, "Invalid JSON syntax", true),
        (r#"{"valid": "json"}"#, "Wrong content type", false),
        ("", "Empty request body", true),
        ("<html>not json</html>", "Invalid JSON content", true),
    ];

    for (payload, error_desc, include_content_type) in error_cases {
        let request = client.post("/test");

        if include_content_type {
            request.clone().header(ContentType::JSON);
        }

        let response = request.body(payload).dispatch();
        assert!(response.status().code >= 400, "Failed for case: {error_desc}");
    }
}

#[test]
fn test_headers_and_methods() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");

    // Test various HTTP methods
    let methods = vec!["GET", "PUT", "DELETE", "HEAD", "PATCH"];
    for method in methods {
        let response = match method {
            "GET" => client.get("/test"),
            "PUT" => client.put("/test"),
            "DELETE" => client.delete("/test"),
            "HEAD" => client.head("/test"),
            "PATCH" => client.patch("/test"),
            _ => unreachable!(),
        }
        .header(ContentType::JSON)
        .body(json!({"test": "data"}).to_string())
        .dispatch();

        assert_eq!(response.status(), Status::NotFound, "Expected NotFound for {method} request");
    }

    // Test POST with various headers
    let custom_headers = vec![
        ("X-Custom-Header", "value"),
        ("User-Agent", "Custom/1.0"),
        ("Accept", "application/json"),
    ];

    for (name, value) in custom_headers {
        let response = client
            .post("/test")
            .header(ContentType::JSON)
            .header(Header::new(name, value))
            .body(json!({"test": "data"}).to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
    }
}

#[test]
fn test_performance() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
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

        let response =
            client.post("/test").header(ContentType::JSON).body(payload.to_string()).dispatch();

        assert_eq!(response.status(), Status::Ok);
    }

    let duration = start.elapsed();
    let avg_response_time = duration.as_micros() as f64 / iterations as f64;

    assert!(avg_response_time < 1000.0, "Average response time too high: {avg_response_time:.2}µs");
}

#[test]
fn test_concurrent_requests() {
    let rocket = create_test_rocket();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(async {
        let client = rocket::local::asynchronous::Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        for i in 0..100 {
            let payload = json!({
                "request_id": i,
                "test": "data",
                "password": "secret",
                "nested": {
                    "credit_card": "4111-1111-1111-1111"
                }
            });

            let response = client
                .post("/test")
                .header(ContentType::JSON)
                .body(payload.to_string())
                .dispatch()
                .await;

            assert_eq!(response.status(), Status::Ok);

            let response_json: Value = response.into_json().await.expect("valid JSON response");

            assert_eq!(response_json, payload);
        }
    });
}
