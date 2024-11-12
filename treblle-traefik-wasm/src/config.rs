use crate::logger::{log, LogLevel};
use serde::{Deserialize, Serialize};
use treblle_core::{Config as CoreConfig, Result, TreblleError};

/// Configuration for the Treblle WASM middleware
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmConfig {
    /// Core Treblle configuration (only api_key is required)
    #[serde(flatten)]
    pub(crate) core: CoreConfig,

    /// Controls response buffering for processing (optional)
    #[serde(default)]
    pub(crate) buffer_response: bool,

    /// Optional path to custom root CA certificate
    #[serde(default)]
    pub(crate) root_ca_path: Option<String>,

    /// Log level for WASM host (optional, defaults to Info)
    #[serde(default)]
    pub(crate) log_level: LogLevel,

    /// Maximum number of connection retries (optional, defaults to 3)
    #[serde(default = "default_max_retries")]
    pub(crate) max_retries: usize,

    /// Maximum size of the connection pool (optional, defaults to 10)
    #[serde(default = "default_max_pool_size")]
    pub(crate) max_pool_size: usize,
}

const DEFAULT_MAX_RETRIES: usize = 3;
const DEFAULT_MAX_POOL_SIZE: usize = 10;

fn default_max_retries() -> usize {
    DEFAULT_MAX_RETRIES
}

fn default_max_pool_size() -> usize {
    DEFAULT_MAX_POOL_SIZE
}

impl WasmConfig {
    /// Create a new WASM configuration builder
    pub fn builder() -> WasmConfigBuilder {
        WasmConfigBuilder::new()
    }

    /// Get configuration from host environment or return error
    pub fn get_or_fallback() -> Result<Self> {
        log(LogLevel::Debug, "Attempting to get configuration from Traefik");

        match Self::get_from_host() {
            Ok(config) => {
                if let Err(e) = config.validate() {
                    log(LogLevel::Error, &format!("Invalid configuration: {e}"));
                    Err(e)
                } else {
                    log(
                        LogLevel::Info,
                        &format!(
                            "Successfully loaded and validated configuration with API key: {}",
                            config.core.api_key,
                        ),
                    );
                    Ok(config)
                }
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Failed to parse config: {e}"));
                Err(e)
            }
        }
    }

    /// Retrieves and parses configuration from the host environment
    fn get_from_host() -> Result<Self> {
        use crate::host_functions::host_get_config;

        let raw_config = host_get_config().map_err(|e| {
            log(LogLevel::Error, &format!("Failed to get config from host: {e}"));
            TreblleError::Config(format!("Failed to get config from host: {e}"))
        })?;

        log(LogLevel::Debug, &format!("Raw config received from host: {raw_config}"));

        serde_json::from_str(&raw_config).map_err(|e| {
            log(LogLevel::Error, &format!("Failed to parse config JSON: {e}"));
            TreblleError::Config(format!("Invalid configuration: {e}"))
        })
    }

    /// Validates the configuration
    pub fn validate(&self) -> Result<()> {
        log(LogLevel::Debug, "Validating configuration...");

        // Only validate that API key is present and not empty
        if self.core.api_key.is_empty() {
            return Err(TreblleError::Config("API key is required".into()));
        }

        log(LogLevel::Debug, "Configuration validation successful");
        Ok(())
    }

    /// Get the log level
    pub fn log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Get whether response buffering is enabled
    pub fn buffer_response(&self) -> bool {
        self.buffer_response
    }

    /// Get the root CA path if configured
    pub fn root_ca_path(&self) -> Option<&str> {
        self.root_ca_path.as_deref()
    }

    /// Get the maximum number of retries
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }

    /// Get the maximum connection pool size
    pub fn max_pool_size(&self) -> usize {
        self.max_pool_size
    }
}

#[derive(Debug, Default)]
pub struct WasmConfigBuilder {
    core_builder: treblle_core::ConfigBuilder,
    buffer_response: Option<bool>,
    root_ca_path: Option<String>,
    log_level: Option<LogLevel>,
    max_retries: Option<usize>,
    max_pool_size: Option<usize>,
}

impl WasmConfigBuilder {
    /// Create a new WASM configuration builder
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Enable or disable response buffering (optional)
    pub fn buffer_response(mut self, buffer: bool) -> Self {
        self.buffer_response = Some(buffer);
        self
    }

    /// Set the root CA path (optional)
    pub fn root_ca_path(mut self, path: impl Into<String>) -> Self {
        self.root_ca_path = Some(path.into());
        self
    }

    /// Set the log level (optional)
    pub fn log_level(mut self, level: LogLevel) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Set the maximum number of retries (optional)
    pub fn max_retries(mut self, retries: usize) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Set the maximum connection pool size (optional)
    pub fn max_pool_size(mut self, size: usize) -> Self {
        self.max_pool_size = Some(size);
        self
    }

    /// Set custom API URLs (optional)
    pub fn set_api_urls(mut self, urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.core_builder = self.core_builder.set_api_urls(urls);
        self
    }

    /// Add additional API URLs (optional)
    pub fn add_api_urls(mut self, urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.core_builder = self.core_builder.add_api_urls(urls);
        self
    }

    /// Add masked fields (optional)
    pub fn add_masked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.add_masked_fields(fields);
        self
    }

    /// Set masked fields (optional)
    pub fn set_masked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.set_masked_fields(fields);
        self
    }

    /// Add regex patterns for masked fields (optional)
    pub fn add_masked_fields_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.add_masked_fields_regex(patterns)?;
        Ok(self)
    }

    /// Set regex patterns for masked fields (optional)
    pub fn set_masked_fields_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.set_masked_fields_regex(patterns)?;
        Ok(self)
    }

    /// Add ignored routes (optional)
    pub fn add_ignored_routes(
        mut self,
        routes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.add_ignored_routes(routes);
        self
    }

    /// Set ignored routes (optional)
    pub fn set_ignored_routes(
        mut self,
        routes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.core_builder = self.core_builder.set_ignored_routes(routes);
        self
    }

    /// Add regex patterns for ignored routes (optional)
    pub fn add_ignored_routes_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.add_ignored_routes_regex(patterns)?;
        Ok(self)
    }

    /// Set regex patterns for ignored routes (optional)
    pub fn set_ignored_routes_regex(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self> {
        self.core_builder = self.core_builder.set_ignored_routes_regex(patterns)?;
        Ok(self)
    }

    /// Build the configuration
    pub fn build(self) -> Result<WasmConfig> {
        Ok(WasmConfig {
            core: self.core_builder.build()?,
            buffer_response: self.buffer_response.unwrap_or_default(),
            root_ca_path: self.root_ca_path,
            log_level: self.log_level.unwrap_or_default(),
            max_retries: self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
            max_pool_size: self.max_pool_size.unwrap_or(DEFAULT_MAX_POOL_SIZE),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_builder_defaults() {
        let config = WasmConfig::builder().api_key("test_key").build().unwrap();

        // Check WASM-specific defaults
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(config.max_pool_size, DEFAULT_MAX_POOL_SIZE);
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(!config.buffer_response);
        assert!(config.root_ca_path.is_none());

        // Check core config defaults
        assert_eq!(config.core.api_key, "test_key");
        assert!(config.core.project_id.is_empty());
    }

    #[test]
    fn test_builder_with_all_fields() {
        let config = WasmConfig::builder()
            .api_key("test_key")
            .project_id("test_project")
            .buffer_response(true)
            .root_ca_path("/path/to/ca.pem")
            .log_level(LogLevel::Debug)
            .max_retries(5)
            .max_pool_size(20)
            .build()
            .unwrap();

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
        assert_eq!(config.root_ca_path, Some("/path/to/ca.pem".to_string()));
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.max_pool_size, 20);
    }

    #[test]
    fn test_camel_case_deserialization() {
        let json = json!({
            "apiKey": "test_key",
            "projectId": "test_project",
            "bufferResponse": true,
            "rootCaPath": "/path/to/ca.pem",
            "logLevel": "debug",
            "maxRetries": 5,
            "maxPoolSize": 20,
            "apiUrls": ["https://custom.api"],
            "maskedFields": ["custom_field"],
            "ignoredRoutes": ["/custom"]
        });

        let config: WasmConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
        assert_eq!(config.root_ca_path, Some("/path/to/ca.pem".to_string()));
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.max_pool_size, 20);
        assert!(config.core.api_urls.contains(&"https://custom.api".to_string()));
        assert!(config.core.should_mask_field("custom_field"));
        assert!(config.core.should_ignore_route("/custom"));
    }

    #[test]
    fn test_snake_case_deserialization() {
        let json = json!({
            "api_key": "test_key",
            "project_id": "test_project",
            "buffer_response": true,
            "root_ca_path": "/path/to/ca.pem",
            "log_level": "debug",
            "max_retries": 5,
            "max_pool_size": 20
        });

        let config: WasmConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
        assert_eq!(config.root_ca_path, Some("/path/to/ca.pem".to_string()));
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.max_pool_size, 20);
    }

    #[test]
    fn test_validation() {
        // Missing API key
        let result = WasmConfig::builder().build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key is required"));

        // Empty API key
        let result = WasmConfig::builder().api_key("").build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key is required"));

        // Valid API key with defaults
        let result = WasmConfig::builder().api_key("test_key").build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_partial_config() {
        let json = json!({
            "apiKey": "test_key",
            "bufferResponse": true
        });

        let config: WasmConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.core.api_key, "test_key");
        assert!(config.buffer_response);
        // Check defaults
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(config.max_pool_size, DEFAULT_MAX_POOL_SIZE);
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(config.root_ca_path.is_none());
    }

    #[test]
    fn test_masked_fields_handling() {
        let config = WasmConfig::builder()
            .api_key("test_key")
            .add_masked_fields(vec!["api_token", "secret"])
            .build()
            .unwrap();

        // Check if fields are properly masked
        assert!(config.core.should_mask_field("api_token"));
        assert!(config.core.should_mask_field("secret"));

        // Test set_masked_fields
        let config = WasmConfig::builder()
            .api_key("test_key")
            .set_masked_fields(vec!["custom_only"])
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("custom_only"));
        assert!(!config.core.should_mask_field("password")); // Default should be gone
    }

    #[test]
    fn test_ignored_routes_handling() {
        let config = WasmConfig::builder()
            .api_key("test_key")
            .add_ignored_routes(vec!["/custom", "/test"])
            .build()
            .unwrap();

        assert!(config.core.should_ignore_route("/custom"));
        assert!(config.core.should_ignore_route("/test"));

        // Test set_ignored_routes
        let config = WasmConfig::builder()
            .api_key("test_key")
            .set_ignored_routes(vec!["/custom_only"])
            .build()
            .unwrap();

        assert!(config.core.should_ignore_route("/custom_only"));
        assert!(!config.core.should_ignore_route("/health")); // Default should be gone
    }

    #[test]
    fn test_api_urls_handling() {
        let config = WasmConfig::builder()
            .api_key("test_key")
            .set_api_urls(vec!["https://custom.api"])
            .build()
            .unwrap();

        assert_eq!(config.core.api_urls, vec!["https://custom.api"]);

        // Test add_api_urls
        let config = WasmConfig::builder()
            .api_key("test_key")
            .add_api_urls(vec!["https://custom.api"])
            .build()
            .unwrap();

        assert!(config.core.api_urls.contains(&"https://custom.api".to_string()));
    }

    #[test]
    fn test_regex_patterns() {
        let config = WasmConfig::builder()
            .api_key("test_key")
            .add_masked_fields_regex(vec!["test_.*"])
            .unwrap()
            .add_ignored_routes_regex(vec!["/test/.*"])
            .unwrap()
            .build()
            .unwrap();

        assert!(config.core.should_mask_field("test_field"));
        assert!(config.core.should_ignore_route("/test/path"));
    }

    #[test]
    fn test_invalid_regex_patterns() {
        let result = WasmConfig::builder()
            .api_key("test_key")
            .add_masked_fields_regex(vec!["["]) // Invalid regex
            .unwrap_err();
        assert!(result.to_string().contains("Invalid masked field regex pattern"));

        let result = WasmConfig::builder()
            .api_key("test_key")
            .add_ignored_routes_regex(vec!["["]) // Invalid regex
            .unwrap_err();
        assert!(result.to_string().contains("Invalid ignored route regex pattern"));
    }
}
