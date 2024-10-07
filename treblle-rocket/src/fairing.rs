use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Data, Request, Response};

pub struct TreblleFairing {
    api_key: String,
    project_id: String,
}

impl TreblleFairing {
    pub(crate) fn new(api_key: String, project_id: String) -> Self {
        Self { api_key, project_id }
    }
}

#[rocket::async_trait]
impl Fairing for TreblleFairing {
    fn info(&self) -> Info {
        Info {
            name: "Treblle Fairing",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, request: &mut Request<'_>, _data: &mut Data<'_>) {
        // TODO: Implement request processing logic
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        // TODO: Implement response processing logic and send data to Treblle
    }
}