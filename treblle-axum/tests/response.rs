use axum::body::Body;
use axum::response::Response;
use http::StatusCode;
use hyper::body::Bytes;
use serde_json::{json, Value};
use std::time::Duration;
use treblle_axum::extractors::AxumExtractor;
use treblle_core::extractors::TreblleExtractor;

fn create_test_response(status: StatusCode, body: &Value) -> Response<Body> {
    let mut res = Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::empty())
        .unwrap();

    res.extensions_mut().insert(Bytes::from(body.to_string()));
    res
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

    let mut res = http::Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap();

    res.extensions_mut().insert(body_bytes);

    let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));
    assert_eq!(info.size, expected_size);
}

#[test]
fn test_extract_response_info() {
    let res = create_test_response(StatusCode::OK, &json!({"result": "success"}));
    let info = AxumExtractor::extract_response_info(&res, Duration::from_secs(1));

    assert_eq!(info.code, 200);
    assert_eq!(info.load_time, 1.0f64);
    assert!(info.body.is_some());
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

    let mut res = http::Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap();

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
