//! Treblle integration for Axum web framework.

mod config;
pub mod extractors;
mod middleware;

use axum::{middleware::from_fn_with_state, Router};
use std::sync::Arc;

pub use config::AxumConfig;
pub use middleware::{treblle_middleware, TreblleLayer};

/// Treblle service for Axum
#[derive(Clone)]
pub struct Treblle {
    pub config: Arc<AxumConfig>,
}

impl Treblle {
    /// Create a new Treblle instance with the API key and default configuration
    pub fn new<T: Into<String>>(api_key: T) -> Self {
        let config = AxumConfig::builder()
            .api_key(api_key)
            .build()
            .expect("Failed to create Treblle configuration");

        Treblle { config: Arc::new(config) }
    }

    /// Create a new Treblle instance from configuration
    pub fn from_config(config: AxumConfig) -> Self {
        Treblle { config: Arc::new(config) }
    }

    /// Create the Treblle middleware layer
    pub fn layer(self) -> TreblleLayer {
        TreblleLayer::new(self.config)
    }
}

/// Extension trait for Router to easily add Treblle middleware
pub trait TreblleExt {
    fn treblle(self, treblle: Treblle) -> Self;
}

impl<S> TreblleExt for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn treblle(self, treblle: Treblle) -> Self {
        let layer = TreblleLayer::new(treblle.config);
        let layer = Arc::new(layer);
        self.layer(from_fn_with_state(layer, treblle_middleware))
    }
}
