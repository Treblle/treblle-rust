//! Treblle integration for Rocket web framework.

mod config;
mod extractors;
mod fairing;

pub use config::RocketConfig;
pub use fairing::TreblleFairing;

/// Treblle service for Rocket
#[derive(Clone)]
pub struct Treblle {
    config: RocketConfig,
}

impl Treblle {
    /// Create a new Treblle instance with the given configuration
    pub fn new(api_key: String, project_id: String) -> Self {
        Treblle {
            config: RocketConfig::new(api_key, project_id),
        }
    }

    /// Add additional fields to mask
    pub fn add_masked_fields(mut self, fields: Vec<String>) -> Self {
        self.config.core.add_masked_fields(fields).unwrap();
        self
    }

    /// Add routes to ignore
    pub fn add_ignored_routes(mut self, routes: Vec<String>) -> Self {
        self.config.core.add_ignored_routes(routes).unwrap();
        self
    }

    /// Create the Treblle fairing
    pub fn layer(self) -> TreblleFairing {
        TreblleFairing::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::{get, local::blocking::Client, routes};

    #[test]
    fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .add_masked_fields(vec!["custom_field".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle
            .config
            .core
            .masked_fields
            .iter()
            .any(|r| r.as_str().contains("custom_field")));
        assert!(treblle
            .config
            .core
            .ignored_routes
            .iter()
            .any(|r| r.as_str().contains("/health")));
    }

    #[test]
    fn test_layer_integration() {
        #[get("/")]
        fn index() -> &'static str {
            "Hello, world!"
        }

        let rocket = rocket::build()
            .attach(
                Treblle::new("api_key".to_string(), "project_id".to_string())
                    .add_masked_fields(vec!["password".to_string()])
                    .add_ignored_routes(vec!["/health".to_string()])
                    .layer(),
            )
            .mount("/", routes![index]);

        let client = Client::tracked(rocket).expect("valid rocket instance");
        let response = client.get("/").dispatch();
        assert_eq!(response.status().code, 200);
    }
}
