use rocket::{
    get,
    http::{ContentType, Status},
    local::blocking::Client,
    post, routes,
    serde::json::Json,
};
use serde_json::{json, Value};
use treblle_rocket::{Treblle, TreblleState};

fn setup_test_rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(Treblle::new("test_key".to_string()).fairing())
        .manage(TreblleState::default())
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

#[test]
fn test_handles_error_responses() {
    let rocket = setup_test_rocket().mount("/", routes![error]);

    let client = Client::tracked(rocket).expect("valid rocket instance");
    let response = client.get("/error").dispatch();

    assert_eq!(response.status(), Status::InternalServerError);
}

#[test]
fn test_handles_validation_errors() {
    let rocket = setup_test_rocket().mount("/", routes![validate]);

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
    let response_body: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
    assert_eq!(response_body["status"], "valid");
}
