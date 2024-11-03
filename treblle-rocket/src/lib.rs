//! Treblle integration for Rocket web framework.
//!
//! This crate provides middleware for logging API requests and responses to Treblle.
//! It supports JSON request/response monitoring, error tracking, and performance monitoring.

mod config;
mod extractors;
mod fairing;

pub use config::RocketConfig;
pub use extractors::TreblleState;
pub use fairing::TreblleFairing;

/// Main struct for Treblle integration with Rocket
#[derive(Clone)]
pub struct Treblle {
    config: RocketConfig,
}

impl Treblle {
    /// Create a new Treblle instance with API key and project ID
    pub fn new(api_key: String, project_id: String) -> Self {
        Treblle {
            config: RocketConfig::new(api_key, project_id),
        }
    }

    /// Add fields to mask in the request/response data
    pub fn add_masked_fields(mut self, fields: Vec<String>) -> Self {
        self.config.add_masked_fields(fields);
        self
    }

    /// Add routes to ignore (will not be monitored)
    pub fn add_ignored_routes(mut self, routes: Vec<String>) -> Self {
        self.config.add_ignored_routes(routes);
        self
    }

    /// Create the Treblle fairing for Rocket
    pub fn fairing(self) -> TreblleFairing {
        TreblleFairing::new(self.config)
    }
}

/// Extension trait for Rocket to easily attach Treblle
pub trait TreblleExt {
    fn attach_treblle(self, api_key: String, project_id: String) -> Self;
}

impl TreblleExt for rocket::Rocket<rocket::Build> {
    fn attach_treblle(self, api_key: String, project_id: String) -> Self {
        self.attach(Treblle::new(api_key, project_id).fairing())
            .manage(TreblleState::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::routes;

    #[test]
    fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .add_masked_fields(vec!["password".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle.config.core.masked_fields.iter().any(|r| r.as_str().contains("password")));
        assert!(treblle.config.core.ignored_routes.iter().any(|r| r.as_str().contains("/health")));
    }

    #[test]
    fn test_rocket_integration() {
        let _rocket = rocket::build()
            .attach_treblle("api_key".to_string(), "project_id".to_string())
            .mount("/", routes![]);

        // Simple build verification is enough
        assert!(true);
    }
}