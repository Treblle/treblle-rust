//! Treblle integration for Actix web framework.

mod config;
mod extractors;
mod middleware;
mod tests;

use actix_web::{dev::Payload, web};
use actix_web::{Error, FromRequest, HttpRequest};
use std::future::{ready, Ready};
pub use config::ActixConfig;
pub use middleware::TreblleMiddleware;

/// Treblle service for Actix
pub struct Treblle {
    config: ActixConfig,
}

impl Treblle {
    /// Create a new Treblle instance with the API key and default configuration
    pub fn new(api_key: String) -> Self {
        let config = ActixConfig::builder()
            .api_key(api_key)
            .build()
            .expect("Failed to create Treblle configuration");

        Treblle { config }
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
            req.app_data::<web::Data<ActixConfig>>()
                .expect("Treblle middleware not configured")
                .get_ref()
                .clone(),
        )))
    }
}
