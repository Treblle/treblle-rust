//! Treblle integration for Rocket web framework.
//!
//! This crate provides middleware for logging API requests and responses to Treblle.
//! It supports JSON request/response monitoring, error tracking, and performance monitoring.

mod config;
mod extractors;
mod fairing;
mod tests;

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