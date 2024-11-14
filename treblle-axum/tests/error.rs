use axum::body::Body;
use axum::response::Response;
use http::StatusCode;
use hyper::body::Bytes;
use serde_json::{json, Value};
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
fn test_extract_error_info() {
    let error_body = json!({
        "error": "Not Found",
        "message": "Resource does not exist"
    });
    let res = create_test_response(StatusCode::NOT_FOUND, &error_body);

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
        let mut res = http::Response::builder().status(status).body(Body::empty()).unwrap();

        res.extensions_mut().insert(Bytes::from(error_body.to_string()));

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
