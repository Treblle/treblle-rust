#[cfg(test)]
pub mod tests {
    use crate::{RocketConfig, Treblle, TreblleState};
    use rocket::{
        get,
        http::{ContentType, Status},
        local::blocking::Client,
        post, routes,
        serde::json::Json,
    };
    use serde_json::{json, Value};

    mod test_utils {
        use super::*;

        // Test route handlers
        #[post("/echo", format = "json", data = "<input>")]
        pub fn echo(input: Json<Value>) -> Json<Value> {
            input
        }

        #[get("/ping")]
        pub fn ping() -> &'static str {
            "pong"
        }

        #[post("/ignored", format = "json", data = "<input>")]
        pub fn ignored(input: Json<Value>) -> Json<Value> {
            input
        }

        #[get("/error")]
        pub fn error() -> Status {
            Status::InternalServerError
        }

        #[post("/validate", format = "json", data = "<input>")]
        pub fn validate(input: Json<Value>) -> Result<Json<Value>, Status> {
            if input.get("valid").and_then(|v| v.as_bool()).unwrap_or(false) {
                Ok(Json(json!({ "status": "valid" })))
            } else {
                Err(Status::BadRequest)
            }
        }

        pub fn setup_test_rocket() -> rocket::Rocket<rocket::Build> {
            rocket::build()
                .attach(Treblle::new("test_key".to_string(), "test_project".to_string()).fairing())
                .manage(TreblleState::default())
        }
    }

    mod config_tests {
        use super::*;

        #[test]
        fn test_rocket_config() {
            let mut config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
            config.add_masked_fields(vec!["password".to_string()]);
            config.add_ignored_routes(vec!["/health".to_string()]);

            assert_eq!(config.core.api_key, "test_key");
            assert_eq!(config.core.project_id, "test_project");
            assert!(config.core.masked_fields.iter().any(|r| r.as_str().contains("password")));
            assert!(config.core.ignored_routes.iter().any(|r| r.as_str().contains("/health")));
        }
    }

    mod request_handling {
        use super::*;
        use test_utils::{echo, ping};

        #[test]
        fn test_processes_json_requests() {
            let rocket = test_utils::setup_test_rocket().mount("/", routes![echo]);

            let client = Client::tracked(rocket).expect("valid rocket instance");

            let test_data = json!({
                "test": "value",
                "nested": {
                    "field": "value"
                }
            });

            let response = client
                .post("/echo")
                .header(ContentType::JSON)
                .body(test_data.to_string())
                .dispatch();

            assert_eq!(response.status(), Status::Ok);
            let response_body: Value =
                serde_json::from_str(&response.into_string().unwrap()).unwrap();
            assert_eq!(response_body, test_data);
        }

        #[test]
        fn test_ignores_non_json_requests() {
            let rocket = test_utils::setup_test_rocket().mount("/", routes![ping]);

            let client = Client::tracked(rocket).expect("valid rocket instance");
            let response = client.get("/ping").dispatch();

            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "pong");
        }
    }

    mod data_masking {
        use super::*;
        use test_utils::echo;

        #[test]
        fn test_masks_sensitive_data() {
            let rocket = rocket::build()
                .attach(
                    Treblle::new("test_key".to_string(), "test_project".to_string())
                        .add_masked_fields(vec!["password".to_string(), "secret".to_string()])
                        .fairing(),
                )
                .manage(TreblleState::default())
                .mount("/", routes![echo]);

            let client = Client::tracked(rocket).expect("valid rocket instance");

            let test_data = json!({
                "username": "test_user",
                "password": "secret123",
                "data": {
                    "secret": "hidden",
                    "visible": "shown"
                }
            });

            let response = client
                .post("/echo")
                .header(ContentType::JSON)
                .body(test_data.to_string())
                .dispatch();

            assert_eq!(response.status(), Status::Ok);
            // Original data should not be masked
            let response_body: Value =
                serde_json::from_str(&response.into_string().unwrap()).unwrap();
            assert_eq!(response_body["password"], "secret123");
            assert_eq!(response_body["data"]["secret"], "hidden");
        }
    }

    mod route_ignoring {
        use super::*;
        use test_utils::{echo, ignored};

        #[test]
        fn test_ignores_specified_routes() {
            let rocket = rocket::build()
                .attach(
                    Treblle::new("test_key".to_string(), "test_project".to_string())
                        .add_ignored_routes(vec!["/ignored.*".to_string()])
                        .fairing(),
                )
                .manage(TreblleState::default())
                .mount("/", routes![echo, ignored]);

            let client = Client::tracked(rocket).expect("valid rocket instance");

            let test_data = json!({
                "test": "value"
            });

            // Test ignored route
            let response = client
                .post("/ignored")
                .header(ContentType::JSON)
                .body(test_data.to_string())
                .dispatch();

            assert_eq!(response.status(), Status::Ok);

            // Test non-ignored route
            let response = client
                .post("/echo")
                .header(ContentType::JSON)
                .body(test_data.to_string())
                .dispatch();

            assert_eq!(response.status(), Status::Ok);
        }
    }

    mod error_handling {
        use super::*;
        use test_utils::{error, validate};

        #[test]
        fn test_handles_error_responses() {
            let rocket = test_utils::setup_test_rocket().mount("/", routes![error]);

            let client = Client::tracked(rocket).expect("valid rocket instance");
            let response = client.get("/error").dispatch();

            assert_eq!(response.status(), Status::InternalServerError);
        }

        #[test]
        fn test_handles_validation_errors() {
            let rocket = test_utils::setup_test_rocket().mount("/", routes![validate]);

            let client = Client::tracked(rocket).expect("valid rocket instance");

            // Test invalid request
            let response = client
                .post("/validate")
                .header(ContentType::JSON)
                .body(json!({ "valid": false }).to_string())
                .dispatch();

            assert_eq!(response.status(), Status::BadRequest);

            // Test valid request
            let response = client
                .post("/validate")
                .header(ContentType::JSON)
                .body(json!({ "valid": true }).to_string())
                .dispatch();

            assert_eq!(response.status(), Status::Ok);
            let response_body: Value =
                serde_json::from_str(&response.into_string().unwrap()).unwrap();
            assert_eq!(response_body["status"], "valid");
        }
    }
}
