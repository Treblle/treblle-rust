//! Treblle integration for Actix web framework.

mod config;
mod extractors;
mod middleware;
mod http_client;

use actix_web::dev::Payload;
use actix_web::web::Data;
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest};
use std::future::{ready, Ready};

pub use config::ActixConfig;
pub use middleware::TreblleMiddleware;

/// Treblle service for Actix
pub struct Treblle {
    config: ActixConfig,
}

impl Treblle {
    /// Create a new Treblle instance with the given configuration
    pub fn new(api_key: String, project_id: String) -> Self {
        Treblle {
            config: ActixConfig::new(api_key, project_id),
        }
    }

    /// Set whether to buffer the response
    pub fn buffer_response(mut self, buffer: bool) -> Self {
        self.config = self.config.buffer_response(buffer);
        self
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

    /// Create the Treblle middleware
    pub fn middleware(self) -> TreblleMiddleware {
        TreblleMiddleware::new(self.config)
    }
}

/// Extractor for accessing Treblle configuration in request handlers
pub struct TreblleConfig(pub ActixConfig);

impl FromRequest for TreblleConfig {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(TreblleConfig(
            req.app_data::<Data<ActixConfig>>()
                .expect("Treblle middleware not configured")
                .get_ref()
                .clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .buffer_response(true)
            .add_masked_fields(vec!["password".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle.config.buffer_response);
        assert!(treblle.config.core.masked_fields.contains(&"password".to_string()));
        assert!(treblle.config.core.ignored_routes.contains(&"/health".to_string()));
    }
}