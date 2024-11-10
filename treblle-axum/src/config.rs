use serde::{Deserialize, Serialize};
use treblle_core::Config as CoreConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AxumConfig {
    #[serde(flatten)]
    pub core: CoreConfig,
    // Add any Axum-specific configuration options here if needed
}

impl AxumConfig {
    pub fn new(api_key: String, project_id: String) -> Self {
        AxumConfig {
            core: CoreConfig::new(api_key, project_id),
        }
    }

    pub fn add_masked_fields(&mut self, fields: Vec<String>) -> &mut Self {
        self.core.add_masked_fields(fields);
        self
    }

    pub fn add_ignored_routes(&mut self, routes: Vec<String>) -> &mut Self {
        self.core.add_ignored_routes(routes);
        self
    }
}

impl Default for AxumConfig {
    fn default() -> Self {
        AxumConfig {
            core: CoreConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axum_config() {
        let mut config = AxumConfig::new("test_key".to_string(), "test_project".to_string());
        config.add_masked_fields(vec!["password".to_string()]);
        config.add_ignored_routes(vec!["/health".to_string()]);

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.core.masked_fields.iter().any(|r| r.as_str().contains("password")));
        assert!(config.core.ignored_routes.iter().any(|r| r.as_str().contains("/health")));
    }
}