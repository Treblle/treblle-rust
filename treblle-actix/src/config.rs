use serde::{Deserialize, Serialize};
use treblle_core::{Config as CoreConfig, Result};

/// Configuration for the Treblle Actix middleware
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActixConfig {
    #[serde(flatten)]
    pub core: CoreConfig,

    /// Controls whether response body should be buffered
    #[serde(default)]
    pub buffer_response: bool,
}

/// Builder for Actix middleware configuration
#[derive(Debug)]
pub struct ActixConfigBuilder {
    core_builder: treblle_core::ConfigBuilder,
    buffer_response: bool,
}

impl ActixConfig {
    /// Create a new configuration builder
    pub fn builder() -> ActixConfigBuilder {
        ActixConfigBuilder { core_builder: CoreConfig::builder(), buffer_response: false }
    }

    /// Get a reference to the core configuration
    pub fn core(&self) -> &CoreConfig {
        &self.core
    }

    /// Check if response buffering is enabled
    pub fn buffer_response(&self) -> bool {
        self.buffer_response
    }
}

impl ActixConfigBuilder {
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

    /// Enable or disable response buffering (optional, defaults to false)
    pub fn buffer_response(mut self, buffer: bool) -> Self {
        self.buffer_response = buffer;
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
    pub fn build(self) -> Result<ActixConfig> {
        Ok(ActixConfig { core: self.core_builder.build()?, buffer_response: self.buffer_response })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let config =
            ActixConfig::builder().api_key("test_key").project_id("test_project").build().unwrap();

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(!config.buffer_response);
    }

    #[test]
    fn test_buffer_response() {
        let config =
            ActixConfig::builder().api_key("test_key").buffer_response(true).build().unwrap();

        assert!(config.buffer_response());
    }

    #[test]
    fn test_builder_masked_fields() {
        let config = ActixConfig::builder()
            .api_key("test_key")
            .add_masked_fields(vec!["custom_field"])
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("custom_field"));
        assert!(config.core.should_mask_field("password")); // Default still works
    }

    #[test]
    fn test_builder_ignored_routes() {
        let config = ActixConfig::builder()
            .api_key("test_key")
            .add_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        assert!(config.core.should_ignore_route("/custom"));
        assert!(config.core.should_ignore_route("/health")); // Default still works
    }

    #[test]
    fn test_builder_set_methods() {
        let config = ActixConfig::builder()
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
        let config = ActixConfig::builder()
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
        let config = ActixConfig::builder()
            .api_key("test_key")
            .project_id("test_project")
            .buffer_response(true)
            .build()
            .unwrap();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ActixConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.core.api_key, "test_key");
        assert_eq!(deserialized.core.project_id, "test_project");
        assert!(deserialized.buffer_response);
    }

    #[test]
    fn test_invalid_config() {
        assert!(ActixConfig::builder().build().is_err()); // Missing API key
        assert!(ActixConfig::builder().api_key("").build().is_err()); // Empty API key
    }
}
