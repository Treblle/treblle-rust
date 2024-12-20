//! Treblle integration for Actix web framework.

mod config;
pub mod extractors;
mod middleware;

use actix_web::{dev::Payload, web};
use actix_web::{Error, FromRequest, HttpRequest};
pub use config::ActixConfig;
pub use middleware::TreblleMiddleware;
use std::future::{ready, Ready};

/// Treblle service for Actix
pub struct Treblle {
    pub config: ActixConfig,
}

impl Treblle {
    /// Create a new Treblle instance with the API key and default configuration
    pub fn new<T: Into<String>>(api_key: T) -> Self {
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
