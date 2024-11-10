//! Treblle integration for Axum web framework.

mod config;
mod extractors;
mod middleware;

use axum::{middleware::from_fn_with_state, Router};
use std::sync::Arc;

pub use config::AxumConfig;
pub use middleware::{TreblleLayer, treblle_middleware};

/// Treblle service for Axum
#[derive(Clone)]
pub struct Treblle {
    config: AxumConfig,
}

impl Treblle {
    /// Create a new Treblle instance with the given configuration
    pub fn new(api_key: String, project_id: String) -> Self {
        Treblle {
            config: AxumConfig::new(api_key, project_id),
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
        let layer = treblle.layer();
        self.layer(from_fn_with_state(Arc::new(layer), treblle_middleware))
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
        assert!(treblle.config.core.masked_fields.iter().any(|r| r.as_str().contains("password")));
        assert!(treblle.config.core.ignored_routes.iter().any(|r| r.as_str().contains("/health")));
    }
}