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
    /// Create a new Treblle instance with the API key and default configuration
    pub fn new<T: Into<String>>(api_key: T) -> Self {
        let config = RocketConfig::builder().api_key(api_key).build().unwrap();

        Treblle { config }
    }

    /// Create a new Treblle instance from configuration
    pub fn from_config(config: RocketConfig) -> Self {
        Treblle { config }
    }

    /// Create the Treblle fairing for Rocket
    pub fn fairing(self) -> TreblleFairing {
        TreblleFairing::new(self.config)
    }
}

/// Extension trait for Rocket to easily attach Treblle
pub trait TreblleExt {
    fn attach_treblle(self, api_key: String) -> Self;
}

impl TreblleExt for rocket::Rocket<rocket::Build> {
    fn attach_treblle(self, api_key: String) -> Self {
        self.attach(Treblle::new(api_key).fairing()).manage(TreblleState::default())
    }
}
