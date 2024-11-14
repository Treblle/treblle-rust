use axum::body::Body;
use http::{HeaderMap, HeaderName, HeaderValue};
use treblle_axum::extractors::AxumExtractor;
use treblle_core::extractors::TreblleExtractor;

fn create_test_request(headers: Vec<(&str, &str)>) -> http::Request<Body> {
    let builder = http::Request::builder().uri("https://api.example.com/test").method("POST");

    let headers_map = headers
        .into_iter()
        .map(|(name, value)| {
            (
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            )
        })
        .collect::<HeaderMap>();

    let req = builder.body(Body::empty()).unwrap();
    let (mut parts, body) = req.into_parts();
    parts.headers = headers_map;
    http::Request::from_parts(parts, body)
}

#[test]
fn test_ip_extraction() {
    let test_cases = vec![
        (vec![("X-Forwarded-For", "203.0.113.195")], "203.0.113.195"),
        (vec![("X-Real-IP", "203.0.113.196")], "203.0.113.196"),
        (vec![("Forwarded", "for=192.0.2.60;proto=http;by=203.0.113.43")], "192.0.2.60"),
        (
            vec![("X-Forwarded-For", "203.0.113.195"), ("X-Real-IP", "203.0.113.196")],
            "203.0.113.195", // X-Forwarded-For should take precedence
        ),
        (
            vec![("Forwarded", "for=192.0.2.60"), ("X-Forwarded-For", "203.0.113.195")],
            "192.0.2.60", // Forwarded header should take precedence
        ),
        (vec![], "unknown"), // No IP headers present
    ];

    for (headers, expected_ip) in test_cases {
        let req = create_test_request(headers);
        let info = AxumExtractor::extract_request_info(&req);
        assert_eq!(info.ip, expected_ip);
    }
}

#[test]
fn test_url_construction() {
    let test_cases = vec![
        (
            vec![("Host", "api.example.com")],
            "https://api.example.com/test?query=value",
            "https://api.example.com/test?query=value",
        ),
        (
            vec![],
            "/test",
            "http:///test", // No host header
        ),
        (vec![("Host", "localhost:8080")], "/api/v1/test", "http://localhost:8080/api/v1/test"),
    ];

    for (headers, uri, expected_url) in test_cases {
        let mut req = create_test_request(headers);
        *req.uri_mut() = uri.parse().unwrap();

        let info = AxumExtractor::extract_request_info(&req);
        assert_eq!(info.url, expected_url);
    }
}

#[test]
fn test_extract_request_info() {
    let req = create_test_request(vec![("Host", "api.example.com"), ("User-Agent", "test-agent")]);
    let info = AxumExtractor::extract_request_info(&req);

    assert_eq!(info.url, "https://api.example.com/test");
    assert_eq!(info.method, "POST");
    assert_eq!(info.user_agent, "test-agent");
}
