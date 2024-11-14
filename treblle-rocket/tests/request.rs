use rocket::serde::json::Json;
use rocket::{
    get,
    http::{ContentType, Status},
    local::blocking::Client,
    post, routes,
};
use serde_json::{json, Value};
use treblle_rocket::{Treblle, TreblleState};

fn setup_test_rocket() -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(Treblle::new("test_key".to_string()).fairing())
        .manage(TreblleState::default())
}

#[post("/echo", format = "json", data = "<input>")]
pub fn echo(input: Json<Value>) -> Json<Value> {
    input
}

#[get("/ping")]
pub fn ping() -> &'static str {
    "pong"
}

#[test]
fn test_processes_json_requests() {
    let rocket = setup_test_rocket().mount("/", routes![echo]);

    let client = Client::tracked(rocket).expect("valid rocket instance");

    let test_data = json!({
        "test": "value",
        "nested": {
            "field": "value"
        }
    });

    let response =
        client.post("/echo").header(ContentType::JSON).body(test_data.to_string()).dispatch();

    assert_eq!(response.status(), Status::Ok);
    let response_body: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
    assert_eq!(response_body, test_data);
}

#[test]
fn test_ignores_non_json_requests() {
    let rocket = setup_test_rocket().mount("/", routes![ping]);

    let client = Client::tracked(rocket).expect("valid rocket instance");
    let response = client.get("/ping").dispatch();

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.into_string().unwrap(), "pong");
}
