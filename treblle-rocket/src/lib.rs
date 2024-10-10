//! Treblle integration for Rocket web framework.

mod config;
mod extractors;
mod fairing;

use rocket::Rocket;
use rocket::Build;

pub use config::RocketConfig;
pub use fairing::TreblleHandler;

/// Treblle service for Rocket
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
        self.config.core.add_masked_fields(fields);
        self
    }

    /// Add routes to ignore
    pub fn add_ignored_routes(mut self, routes: Vec<String>) -> Self {
        self.config.core.add_ignored_routes(routes);
        self
    }

    /// Attach the Treblle handler to a Rocket instance
    pub fn attach(self, rocket: Rocket<Build>) -> Rocket<Build> {
        rocket.attach(TreblleHandler::new(self.config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .add_masked_fields(vec!["password".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle.config.core.masked_fields.contains(&"password".to_string()));
        assert!(treblle.config.core.ignored_routes.iter().any(|r| r.as_str() == "/health"));
    }
}