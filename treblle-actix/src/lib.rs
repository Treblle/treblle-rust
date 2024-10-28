//! Treblle integration for Actix web framework.

mod config;
mod extractors;
mod middleware;

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
    /// Create a new Treblle instance with the given configuration
    pub fn new(api_key: String, project_id: String) -> Self {
        Treblle {
            config: ActixConfig::new(api_key, project_id),
        }
    }

    /// Add additional fields to mask
    pub fn add_masked_fields(mut self, fields: Vec<String>) -> Self {
        self.config.core.add_masked_fields(fields).expect("Invalid masked field pattern");
        self
    }

    /// Add routes to ignore
    pub fn add_ignored_routes(mut self, routes: Vec<String>) -> Self {
        self.config.core.add_ignored_routes(routes).expect("Invalid route pattern");
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
            req.app_data::<web::Data<ActixConfig>>()
                .expect("Treblle middleware not configured")
                .get_ref()
                .clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, web};

    #[actix_web::test]
    async fn test_treblle_builder() {
        let treblle = Treblle::new("api_key".to_string(), "project_id".to_string())
            .add_masked_fields(vec!["custom_field".to_string()])
            .add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(treblle.config.core.api_key, "api_key");
        assert_eq!(treblle.config.core.project_id, "project_id");
        assert!(treblle.config.core.masked_fields.iter()
            .any(|r| r.as_str().contains("custom_field")));
        assert!(treblle.config.core.ignored_routes.iter()
            .any(|r| r.as_str().contains("/health")));
    }

    #[actix_web::test]
    async fn test_treblle_config_extraction() {
        let config = ActixConfig::new("test_key".to_string(), "test_project".to_string());

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config.clone()))
                .route("/test", web::get().to(|| async { "ok" }))
        ).await;

        let req = test::TestRequest::default()
            .to_request();

        let srv_req = test::call_service(&app, req).await;
        let config_extracted = TreblleConfig::extract(&srv_req.request()).await.unwrap();

        assert_eq!(config_extracted.0.core.api_key, config.core.api_key);
        assert_eq!(config_extracted.0.core.project_id, config.core.project_id);
    }
}