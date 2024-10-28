use rocket::{
    fairing::{Fairing, Info, Kind},
    Data, Request, Response,
};
use std::{sync::Arc, time::Instant};
use tracing::{debug, error};
use treblle_core::{PayloadBuilder, TreblleClient};

use crate::{config::RocketConfig, extractors::RocketExtractor};

#[derive(Copy, Clone)]
struct RequestStart(Instant);

pub struct TreblleFairing {
    config: Arc<RocketConfig>,
    treblle_client: Arc<TreblleClient>,
}

impl TreblleFairing {
    pub fn new(config: RocketConfig) -> Self {
        TreblleFairing {
            treblle_client: Arc::new(
                TreblleClient::new(config.core.clone()).expect("Failed to create Treblle client"),
            ),
            config: Arc::new(config),
        }
    }
}

#[rocket::async_trait]
impl Fairing for TreblleFairing {
    fn info(&self) -> Info {
        Info {
            name: "Treblle Layer",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, request: &mut Request<'_>, data: &mut Data<'_>) {
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();

        request.local_cache(|| RequestStart(Instant::now()));

        let should_process = !config
            .core
            .ignored_routes
            .iter()
            .any(|route| route.is_match(&request.uri().path().to_string()))
            && request
                .content_type()
                .map(|ct| ct.is_json())
                .unwrap_or(false);

        if should_process {
            debug!("Processing request for Treblle: {}", request.uri());

            // Read and store the request body
            if let Some(body) = data.peek().await {
                request.local_cache(|| body.to_vec());
            }

            let request_payload =
                PayloadBuilder::build_request_payload::<RocketExtractor<'_>>(request, &config.core);

            rocket::tokio::spawn(async move {
                if let Err(e) = treblle_client.send_to_treblle(request_payload).await {
                    error!("Failed to send request payload to Treblle: {:?}", e);
                }
            });
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        let config = self.config.clone();
        let treblle_client = self.treblle_client.clone();

        let should_process = !config
            .core
            .ignored_routes
            .iter()
            .any(|route| route.is_match(&request.uri().path().to_string()))
            && request
                .content_type()
                .map(|ct| ct.is_json())
                .unwrap_or(false);

        if should_process {
            let start_time = request
                .local_cache(|| RequestStart(Instant::now()))
                .0
                .elapsed();

            debug!("Processing response for Treblle: {}", response.status());

            // Store response body in local cache
            if let Body::Sized(cursor) = response.body() {
                let mut buf = Vec::new();
                if cursor.read_to_end(&mut buf).is_ok() {
                    response.local_cache(|| buf);
                }
            }

            let response_payload = PayloadBuilder::build_response_payload::<RocketExtractor<'_>>(
                response,
                &config.core,
                start_time,
            );

            rocket::tokio::spawn(async move {
                if let Err(e) = treblle_client.send_to_treblle(response_payload).await {
                    error!("Failed to send response payload to Treblle: {:?}", e);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::{
        get,
        http::{ContentType, Status},
        local::blocking::Client,
        post, routes,
        serde::json::{json, Json, Value},
    };

    #[post("/echo", format = "json", data = "<body>")]
    fn echo_handler(body: Json<Value>) -> Json<Value> {
        body
    }

    fn setup_test_config() -> RocketConfig {
        let mut config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec![
                "password".to_string(),
                "Password".to_string(),
                "user_password".to_string(),
                "credit_card".to_string(),
                "cvv".to_string(),
                "ssn".to_string(),
            ])
            .expect("Failed to add masked fields");
        config
    }

    #[test]
    fn test_data_masking_patterns() {
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
            let config = setup_test_config();
            let rocket = rocket::build()
                .mount("/", routes![echo_handler])
                .attach(TreblleFairing::new(config.clone()));

            let client = Client::tracked(rocket).expect("valid rocket instance");

            let payload_bytes = payload.to_string();

            // Create request for testing
            let req = client
                .post("/echo")
                .header(ContentType::JSON)
                .body(payload_bytes.clone());

            // Test Treblle payload masking
            let treblle_request = req.inner();
            let treblle_payload = PayloadBuilder::build_request_payload::<RocketExtractor>(
                &treblle_request,
                &config.core,
            );

            // Verify masking in Treblle payload
            if let Some(body) = treblle_payload.data.request.body {
                for &field in &fields_to_check {
                    if let Some(value) = find_field_value(&body, field) {
                        assert_eq!(value, "*****", "Field '{}' was not properly masked", field);
                    }
                }
            }

            // Test that original response is unmasked
            let response = req.dispatch();
            assert_eq!(response.status(), Status::Ok);

            let body: Value = response.into_json().expect("valid JSON response");

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

    #[test]
    fn test_partial_field_masking() {
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

        let mut config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec!["number".to_string(), "cvv".to_string()])
            .expect("Failed to add masked fields");

        let rocket = rocket::build()
            .mount("/", routes![echo_handler])
            .attach(TreblleFairing::new(config.clone()));

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let payload_bytes = test_data.to_string();

        // Create request for testing
        let req = client
            .post("/echo")
            .header(ContentType::JSON)
            .body(payload_bytes.clone());

        // Test Treblle payload masking
        let treblle_request = req.inner();
        let treblle_payload = PayloadBuilder::build_request_payload::<RocketExtractor>(
            &treblle_request,
            &config.core,
        );

        // Verify masking in Treblle payload
        if let Some(body) = treblle_payload.data.request.body {
            assert_eq!(body["user"]["email"].as_str().unwrap(), "test@example.com");
            assert_eq!(
                body["user"]["credit_card"]["number"].as_str().unwrap(),
                "*****"
            );
            assert_eq!(
                body["user"]["credit_card"]["expiry"].as_str().unwrap(),
                "12/24"
            );
            assert_eq!(
                body["user"]["credit_card"]["cvv"].as_str().unwrap(),
                "*****"
            );
            assert_eq!(
                body["user"]["shipping_address"]["street"].as_str().unwrap(),
                "123 Main St"
            );
        }

        // Test that original response is unmasked
        let response = req.dispatch();
        assert_eq!(response.status(), Status::Ok);

        let body: Value = response.into_json().expect("valid JSON response");

        // Verify original data is preserved
        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["credit_card"]["number"], "4111-1111-1111-1111");
        assert_eq!(body["user"]["credit_card"]["expiry"], "12/24");
        assert_eq!(body["user"]["credit_card"]["cvv"], "123");
        assert_eq!(body["user"]["shipping_address"]["street"], "123 Main St");
    }

    // Helper function to recursively find a field value in a JSON object
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

    #[test]
    fn test_treblle_payload_creation() {
        let mut config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_masked_fields(vec!["password".to_string()])
            .unwrap();

        let test_data = json!({
            "username": "test_user",
            "password": "secret123"
        });

        let rocket = rocket::build()
            .mount("/", routes![echo_handler])
            .attach(TreblleFairing::new(config.clone()));

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let req = client
            .post("/echo")
            .header(ContentType::JSON)
            .body(test_data.to_string());

        let treblle_request = req.inner();
        let payload = PayloadBuilder::build_request_payload::<RocketExtractor>(
            &treblle_request,
            &config.core,
        );

        if let Some(body) = &payload.data.request.body {
            assert_eq!(body["username"].as_str().unwrap(), "test_user");
            assert_eq!(body["password"].as_str().unwrap(), "*****");
        }
    }

    #[get("/text")]
    fn text_handler() -> &'static str {
        "Hello, World!"
    }

    #[post("/ignored", format = "json", data = "<body>")]
    fn ignored_handler(body: Json<Value>) -> Json<Value> {
        body
    }

    #[test]
    fn test_middleware_allows_non_json_requests() {
        let config = setup_test_config();
        let rocket = rocket::build()
            .mount("/", routes![text_handler])
            .attach(TreblleFairing::new(config));

        let client = Client::tracked(rocket).expect("valid rocket instance");
        let response = client.get("/text").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "Hello, World!");
    }

    #[test]
    fn test_middleware_respects_ignored_routes() {
        let mut config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
        config
            .core
            .add_ignored_routes(vec!["/ignored.*".to_string()])
            .unwrap();
        config
            .core
            .add_masked_fields(vec!["password".to_string()])
            .unwrap();

        let rocket = rocket::build()
            .mount("/", routes![echo_handler, ignored_handler]) // Mount both handlers
            .attach(TreblleFairing::new(config));

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let test_data = json!({
            "password": "secret123",
            "credit_card": "4111-1111-1111-1111"
        });

        let response = client
            .post("/ignored")
            .header(ContentType::JSON)
            .body(test_data.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);

        let body: Value = response.into_json().unwrap();
        assert_eq!(body["password"], "secret123");
        assert_eq!(body["credit_card"], "4111-1111-1111-1111");
    }

    #[test]
    fn test_middleware_preserves_original_data() {
        let config = setup_test_config();
        let rocket = rocket::build()
            .mount("/", routes![echo_handler])
            .attach(TreblleFairing::new(config));

        let client = Client::tracked(rocket).expect("valid rocket instance");

        let test_data = json!({
            "user": {
                "email": "test@example.com",
                "password": "secret123",
                "credit_card": "4111-1111-1111-1111"
            }
        });

        let response = client
            .post("/echo")
            .header(ContentType::JSON)
            .body(test_data.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);

        let body: Value = response.into_json().unwrap();
        assert_eq!(body["user"]["email"], "test@example.com");
        assert_eq!(body["user"]["password"], "secret123");
        assert_eq!(body["user"]["credit_card"], "4111-1111-1111-1111");
    }
}
