use actix_http::{header, HttpMessage};
use actix_web::dev::ServiceResponse;
use actix_web::{test, HttpResponse};
use bytes::Bytes;
use serde_json::json;
use std::time::Duration;
use treblle_actix::extractors::ActixExtractor;
use treblle_core::extractors::TreblleExtractor;

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
