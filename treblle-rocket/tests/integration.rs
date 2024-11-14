use rocket::{
    http::{ContentType, Status},
    local::blocking::Client,
    post, routes,
    serde::json::Json,
};
use serde_json::{json, Value};
use treblle_rocket::{RocketConfig, Treblle, TreblleState};

#[post("/echo", format = "json", data = "<input>")]
pub fn echo(input: Json<Value>) -> Json<Value> {
    input
}

#[post("/ignored", format = "json", data = "<input>")]
pub fn ignored(input: Json<Value>) -> Json<Value> {
    input
}

#[test]
fn test_masks_sensitive_data() {
    let rocket = rocket::build()
        .attach(Treblle::new("test_key".to_string()).fairing())
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

    let response =
        client.post("/echo").header(ContentType::JSON).body(test_data.to_string()).dispatch();

    assert_eq!(response.status(), Status::Ok);
    // Original data should not be masked
    let response_body: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
    assert_eq!(response_body["password"], "secret123");
    assert_eq!(response_body["data"]["secret"], "hidden");
}

#[test]
fn test_ignores_specified_routes() {
    let config = RocketConfig::builder()
        .api_key("test_key")
        .add_ignored_routes(vec!["/ignored.*".to_string()])
        .build()
        .unwrap();

    let rocket = rocket::build()
        .attach(Treblle::from_config(config).fairing())
        .manage(TreblleState::default())
        .mount("/", routes![echo, ignored]);

    let client = Client::tracked(rocket).expect("valid rocket instance");

    let test_data = json!({
        "test": "value"
    });

    // Test ignored route
    let response =
        client.post("/ignored").header(ContentType::JSON).body(test_data.to_string()).dispatch();

    assert_eq!(response.status(), Status::Ok);

    // Test non-ignored route
    let response =
        client.post("/echo").header(ContentType::JSON).body(test_data.to_string()).dispatch();

    assert_eq!(response.status(), Status::Ok);
}
