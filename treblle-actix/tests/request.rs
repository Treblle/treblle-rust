use actix_web::{test, web, App, HttpResponse};
use futures_util::FutureExt;
use treblle_actix::extractors::ActixExtractor;
use treblle_core::extractors::TreblleExtractor;

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

#[actix_web::test]
async fn test_url_construction() {
    let test_cases = vec![
        (
            vec![("Host", "api.example.com"), ("X-Forwarded-Proto", "https")],
            "/test",
            "https://api.example.com/test",
        ),
        (vec![("Host", "localhost:8080")], "/api/v1/users", "http://localhost:8080/api/v1/users"),
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
    let req = create_test_request(vec![("User-Agent", "test-agent"), ("Host", "localhost:8080")]);

    let info = ActixExtractor::extract_request_info(&req);

    assert_eq!(info.url, "http://localhost:8080/test");
    assert_eq!(info.method, "GET");
    assert_eq!(info.user_agent, "test-agent");
}
