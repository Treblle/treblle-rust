#[cfg(test)]
pub mod tests {
    use rocket::{
        http::{ContentType, Status},
        local::blocking::Client,
        post, get,
        serde::json::Json,
        routes,
    };
    use serde_json::{json, Value};
    use crate::{Treblle, TreblleState};

    // Test route handlers
    #[post("/echo", format = "json", data = "<input>")]
    fn echo(input: Json<Value>) -> Json<Value> {
        input
    }

    #[get("/ping")]
    fn ping() -> &'static str {
        "pong"
    }

    #[post("/ignored", format = "json", data = "<input>")]
    fn ignored(input: Json<Value>) -> Json<Value> {
        input
    }

    #[get("/error")]
    fn error() -> Status {
        Status::InternalServerError
    }

    #[post("/validate", format = "json", data = "<input>")]
    fn validate(input: Json<Value>) -> Result<Json<Value>, Status> {
        if input.get("valid").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(Json(json!({ "status": "valid" })))
        } else {
            Err(Status::BadRequest)
        }
    }

    mod request_handling {
        use super::*;

        #[test]
        fn test_processes_json_requests() {
            let rocket = rocket::build()
                .attach(Treblle::new("test_key".to_string(), "test_project".to_string()).fairing())
                .manage(TreblleState::default())
                .mount("/", routes![echo]);

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
            let response_body: Value = serde_json::from_str(
                &response.into_string().unwrap()
            ).unwrap();
            assert_eq!(response_body, test_data);
        }

        #[test]
        fn test_ignores_non_json_requests() {
            let rocket = rocket::build()
                .attach(Treblle::new("test_key".to_string(), "test_project".to_string()).fairing())
                .manage(TreblleState::default())
                .mount("/", routes![ping]);

            let client = Client::tracked(rocket).expect("valid rocket instance");
            let response = client.get("/ping").dispatch();

            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "pong");
        }
    }

    mod data_masking {
        use super::*;

        #[test]
        fn test_masks_sensitive_data() {
            let rocket = rocket::build()
                .attach(
                    Treblle::new("test_key".to_string(), "test_project".to_string())
                        .add_masked_fields(vec!["password".to_string(), "secret".to_string()])
                        .fairing()
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
            let response_body: Value = serde_json::from_str(
                &response.into_string().unwrap()
            ).unwrap();
            assert_eq!(response_body["password"], "secret123");
            assert_eq!(response_body["data"]["secret"], "hidden");
        }
    }

    mod route_ignoring {
        use super::*;

        #[test]
        fn test_ignores_specified_routes() {
            let rocket = rocket::build()
                .attach(
                    Treblle::new("test_key".to_string(), "test_project".to_string())
                        .add_ignored_routes(vec!["/ignored.*".to_string()])
                        .fairing()
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

        #[test]
        fn test_handles_error_responses() {
            let rocket = rocket::build()
                .attach(Treblle::new("test_key".to_string(), "test_project".to_string()).fairing())
                .manage(TreblleState::default())
                .mount("/", routes![error]);

            let client = Client::tracked(rocket).expect("valid rocket instance");
            let response = client.get("/error").dispatch();

            assert_eq!(response.status(), Status::InternalServerError);
        }

        #[test]
        fn test_handles_validation_errors() {
            let rocket = rocket::build()
                .attach(Treblle::new("test_key".to_string(), "test_project".to_string()).fairing())
                .manage(TreblleState::default())
                .mount("/", routes![validate]);

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
            let response_body: Value = serde_json::from_str(
                &response.into_string().unwrap()
            ).unwrap();
            assert_eq!(response_body["status"], "valid");
        }
    }
}