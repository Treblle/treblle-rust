use actix_http::{HttpMessage, StatusCode};
use actix_web::dev::ServiceResponse;
use actix_web::{test, HttpResponse};
use bytes::Bytes;
use serde_json::json;
use treblle_actix::extractors::ActixExtractor;
use treblle_core::extractors::TreblleExtractor;

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
        HttpResponse::NotFound().content_type("application/json").body(error_body.to_string()),
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
