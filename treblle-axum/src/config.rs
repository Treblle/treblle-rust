use serde::{Deserialize, Serialize};
use treblle_core::{Config as CoreConfig, Result};

/// Configuration for the Treblle Axum middleware
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxumConfig {
    #[serde(flatten)]
    pub(crate) core: CoreConfig,
}

/// Builder for Axum middleware configuration
#[derive(Debug)]
pub struct AxumConfigBuilder {
    core_builder: treblle_core::ConfigBuilder,
}

impl AxumConfig {
    /// Create a new configuration builder
    pub fn builder() -> AxumConfigBuilder {
        AxumConfigBuilder { core_builder: CoreConfig::builder() }
    }

    /// Get a reference to the core configuration
    pub fn core(&self) -> &CoreConfig {
        &self.core
    }
}

impl AxumConfigBuilder {
    /// Set the API key (required)
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.core_builder = self.core_builder.api_key(key);
        self
    }

    /// Set the project ID (optional)
    pub fn project_id(mut self, id: impl Into<String>) -> Self {
        self.core_builder = self.core_builder.project_id(id);
        self
    }

    /// Set custom API URLs (optional)
    pub fn set_api_urls(mut self, urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.core_builder = self.core_builder.set_api_urls(urls);
        self
    }

    /// Add additional API URLs to the default set
    pub fn add_api_urls(mut self, urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.core_builder = self.core_builder.add_api_urls(urls);
        self
    }

    /// Add masked fields to the default set
    pub fn add_masked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.add_masked_fields(fields);
        self
    }

    /// Set masked fields, replacing the defaults
    pub fn set_masked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.set_masked_fields(fields);
        self
    }

    /// Add regex patterns for masked fields to the default set
    pub fn add_masked_fields_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.add_masked_fields_regex(patterns)?;
        Ok(self)
    }

    /// Set regex patterns for masked fields, replacing the defaults
    pub fn set_masked_fields_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.set_masked_fields_regex(patterns)?;
        Ok(self)
    }

    /// Add ignored routes to the default set
    pub fn add_ignored_routes(
        mut self,
        routes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.add_ignored_routes(routes);
        self
    }

    /// Set ignored routes, replacing the defaults
    pub fn set_ignored_routes(
        mut self,
        routes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.set_ignored_routes(routes);
        self
    }

    /// Add regex patterns for ignored routes to the default set
    pub fn add_ignored_routes_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.add_ignored_routes_regex(patterns)?;
        Ok(self)
    }

    /// Set regex patterns for ignored routes, replacing the defaults
    pub fn set_ignored_routes_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.set_ignored_routes_regex(patterns)?;
        Ok(self)
    }

    /// Build the configuration
    pub fn build(self) -> Result<AxumConfig> {
        Ok(AxumConfig { core: self.core_builder.build()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_builder_basic() {
        let config =
            AxumConfig::builder().api_key("test_key").project_id("test_project").build().unwrap();

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
    }

    #[test]
    fn test_builder_masked_fields() {
        let config = AxumConfig::builder()
            .api_key("test_key")
            .add_masked_fields(vec!["custom_field"])
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("custom_field"));
        assert!(config.core.should_mask_field("password")); // Default still works
    }

    #[test]
    fn test_builder_ignored_routes() {
        let config = AxumConfig::builder()
            .api_key("test_key")
            .add_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        assert!(config.core.should_ignore_route("/custom"));
        assert!(config.core.should_ignore_route("/health")); // Default still works
    }

    #[test]
    fn test_builder_set_methods() {
        let config = AxumConfig::builder()
            .api_key("test_key")
            .set_masked_fields(vec!["custom_field"])
            .set_masked_fields_regex(vec!["custom_.*"])
            .unwrap()
            .set_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("custom_field"));
        assert!(!config.core.should_mask_field("password")); // Default gone
        assert!(config.core.should_ignore_route("/custom"));
        assert!(!config.core.should_ignore_route("/health")); // Default gone
    }

    #[test]
    fn test_builder_regex_patterns() {
        let config = AxumConfig::builder()
            .api_key("test_key")
            .add_masked_fields_regex(vec!["test_.*"])
            .unwrap()
            .add_ignored_routes_regex(vec!["/test/.*"])
            .unwrap()
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("test_field"));
        assert!(config.core.should_ignore_route("/test/route"));
    }

    #[test]
    fn test_serialization() {
        let config =
            AxumConfig::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AxumConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.core.api_key, "test_key");
        assert_eq!(deserialized.core.project_id, "test_project");
    }

    #[test]
    fn test_camel_case_deserialization() {
        let json = json!({
            "apiKey": "test_key",
            "projectId": "test_project",
            "apiUrls": ["https://custom.api"],
            "maskedFields": ["custom_field"],
            "ignoredRoutes": ["/custom"]
        });

        let config: AxumConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.core.api_urls.contains(&"https://custom.api".to_string()));
        assert!(config.core.should_mask_field("custom_field"));
        assert!(config.core.should_ignore_route("/custom"));
    }

    #[test]
    fn test_invalid_config() {
        assert!(AxumConfig::builder().build().is_err()); // Missing API key
        assert!(AxumConfig::builder().api_key("").build().is_err()); // Empty API key
    }
}
