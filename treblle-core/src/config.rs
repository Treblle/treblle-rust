use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::constants::defaults::{
    API_URLS, DEFAULT_IGNORED_ROUTES, DEFAULT_IGNORED_ROUTES_REGEX, DEFAULT_MASKED_FIELDS,
    DEFAULT_MASKED_FIELDS_REGEX,
};
use crate::error::{Result, TreblleError};

/// Configuration builder for Treblle integrations
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    api_key: Option<String>,
    project_id: Option<String>,
    api_urls: Option<Vec<String>>,
    masked_fields: Option<HashSet<String>>,
    masked_fields_regex: Option<Vec<Regex>>,
    ignored_routes: Option<HashSet<String>>,
    ignored_routes_regex: Option<Vec<Regex>>,
}

impl ConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            api_key: None,
            project_id: None,
            api_urls: None,
            masked_fields: None,
            masked_fields_regex: None,
            ignored_routes: None,
            ignored_routes_regex: None,
        }
    }

    /// Set the API key (required)
    pub fn api_key<T: Into<String>>(mut self, key: T) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the project ID (optional, defaults to empty string)
    pub fn project_id<T: Into<String>>(mut self, id: T) -> Self {
        self.project_id = Some(id.into());
        self
    }

    /// Set custom API URLs (optional, uses default URLs if not set)
    pub fn set_api_urls<T: Into<String>, I: IntoIterator<Item = T>>(mut self, urls: I) -> Self {
        self.api_urls = Some(urls.into_iter().map(Into::into).collect());
        self
    }

    /// Add additional API URLs to the default set
    pub fn add_api_urls<T: Into<String>, I: IntoIterator<Item = T>>(mut self, urls: I) -> Self {
        if let Some(existing) = &mut self.api_urls {
            existing.extend(urls.into_iter().map(Into::into));
        } else {
            let mut defaults: Vec<_> = API_URLS.iter().map(ToString::to_string).collect();
            defaults.extend(urls.into_iter().map(Into::into));
            self.api_urls = Some(defaults);
        }
        self
    }

    /// Add masked fields to the default set
    pub fn add_masked_fields<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        fields: I,
    ) -> Self {
        let fields: HashSet<String> = fields.into_iter().map(Into::into).collect();

        if let Some(existing) = &mut self.masked_fields {
            existing.extend(fields);
        } else {
            let mut defaults: HashSet<_> =
                DEFAULT_MASKED_FIELDS.iter().map(ToString::to_string).collect();
            defaults.extend(fields);
            self.masked_fields = Some(defaults);
        }
        self
    }

    /// Set masked fields, replacing the defaults
    pub fn set_masked_fields<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        fields: I,
    ) -> Self {
        let new_fields = fields.into_iter().map(Into::into).collect();
        self.masked_fields = Some(new_fields);
        self
    }

    /// Add regex patterns for masked fields to the default set
    pub fn add_masked_fields_regex<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        patterns: I,
    ) -> Result<Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p.into()).map_err(|e| {
                    TreblleError::Config(format!("Invalid masked field regex pattern: {e}"))
                })
            })
            .collect();

        if let Some(existing) = &mut self.masked_fields_regex {
            existing.extend(new_patterns?);
        } else {
            let mut defaults = vec![Regex::new(DEFAULT_MASKED_FIELDS_REGEX)
                .expect("Default masked fields regex is invalid")];
            defaults.extend(new_patterns?);
            self.masked_fields_regex = Some(defaults);
        }
        Ok(self)
    }

    /// Set regex patterns for masked fields, replacing the defaults
    pub fn set_masked_fields_regex<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        patterns: I,
    ) -> Result<Self> {
        let compiled: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p.into()).map_err(|e| {
                    TreblleError::Config(format!("Invalid masked field regex pattern: {e}"))
                })
            })
            .collect();
        self.masked_fields_regex = Some(compiled?);
        Ok(self)
    }

    /// Add ignored routes to the default set
    pub fn add_ignored_routes<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        routes: I,
    ) -> Self {
        let routes: HashSet<String> = routes.into_iter().map(Into::into).collect();

        if let Some(existing) = &mut self.ignored_routes {
            existing.extend(routes);
        } else {
            let mut defaults: HashSet<_> =
                DEFAULT_IGNORED_ROUTES.iter().map(ToString::to_string).collect();
            defaults.extend(routes);
            self.ignored_routes = Some(defaults);
        }
        self
    }

    /// Set ignored routes, replacing the defaults
    pub fn set_ignored_routes<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        routes: I,
    ) -> Self {
        let new_routes = routes.into_iter().map(Into::into).collect();
        self.ignored_routes = Some(new_routes);
        self
    }

    /// Add regex patterns for ignored routes to the default set
    pub fn add_ignored_routes_regex<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        patterns: I,
    ) -> Result<Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p.into()).map_err(|e| {
                    TreblleError::Config(format!("Invalid ignored route regex pattern: {e}"))
                })
            })
            .collect();

        if let Some(existing) = &mut self.ignored_routes_regex {
            existing.extend(new_patterns?);
        } else {
            let mut defaults = vec![Regex::new(DEFAULT_IGNORED_ROUTES_REGEX)
                .expect("Default ignored routes regex is invalid")];
            defaults.extend(new_patterns?);
            self.ignored_routes_regex = Some(defaults);
        }
        Ok(self)
    }

    /// Set regex patterns for ignored routes, replacing the defaults
    pub fn set_ignored_routes_regex<T: Into<String>, I: IntoIterator<Item = T>>(
        mut self,
        patterns: I,
    ) -> Result<Self> {
        let compiled: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p.into()).map_err(|e| {
                    TreblleError::Config(format!("Invalid ignored route regex pattern: {e}"))
                })
            })
            .collect();
        self.ignored_routes_regex = Some(compiled?);
        Ok(self)
    }

    /// Build the configuration
    pub fn build(self) -> Result<Config> {
        let api_key =
            self.api_key.ok_or_else(|| TreblleError::Config("API key is required".into()))?;

        if api_key.is_empty() {
            return Err(TreblleError::Config("API key cannot be empty".into()));
        }

        Ok(Config {
            api_key,
            project_id: self.project_id.unwrap_or_default(),
            api_urls: self
                .api_urls
                .unwrap_or_else(|| API_URLS.iter().map(ToString::to_string).collect()),
            masked_fields: self
                .masked_fields
                .unwrap_or_else(|| DEFAULT_MASKED_FIELDS.iter().map(ToString::to_string).collect()),
            masked_fields_regex: self.masked_fields_regex.unwrap_or_else(|| {
                vec![Regex::new(DEFAULT_MASKED_FIELDS_REGEX)
                    .expect("Default masked fields regex is invalid")]
            }),
            ignored_routes: self.ignored_routes.unwrap_or_else(|| {
                DEFAULT_IGNORED_ROUTES.iter().map(ToString::to_string).collect()
            }),
            ignored_routes_regex: self.ignored_routes_regex.unwrap_or_else(|| {
                vec![Regex::new(DEFAULT_IGNORED_ROUTES_REGEX)
                    .expect("Default ignored routes regex is invalid")]
            }),
        })
    }
}

/// Configuration for Treblle integrations
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The Treblle API key (required)
    pub api_key: String,

    /// The Treblle project ID (optional)
    #[serde(default)]
    pub project_id: String,

    /// The base URLs for the Treblle API (optional, defaults to predefined URLs)
    #[serde(default = "default_api_urls")]
    pub api_urls: Vec<String>,

    /// Fields to mask in request/response data (exact matches)
    #[serde(default = "default_masked_fields")]
    pub masked_fields: HashSet<String>,

    /// Regex patterns for fields to mask
    #[serde(skip)]
    pub masked_fields_regex: Vec<Regex>,

    /// Routes to ignore (exact matches)
    #[serde(default = "default_ignored_routes")]
    pub ignored_routes: HashSet<String>,

    /// Regex patterns for routes to ignore
    #[serde(skip)]
    pub ignored_routes_regex: Vec<Regex>,
}

// Default functions for serde
fn default_api_urls() -> Vec<String> {
    API_URLS.iter().map(ToString::to_string).collect()
}

fn default_masked_fields() -> HashSet<String> {
    DEFAULT_MASKED_FIELDS.iter().map(ToString::to_string).collect()
}

fn default_ignored_routes() -> HashSet<String> {
    DEFAULT_IGNORED_ROUTES.iter().map(ToString::to_string).collect()
}

impl Config {
    /// Create a new configuration builder
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Check if a field should be masked
    pub fn should_mask_field(&self, field: &str) -> bool {
        self.masked_fields.contains(field)
            || self.masked_fields_regex.iter().any(|re| re.is_match(field))
    }

    /// Check if a route should be ignored
    pub fn should_ignore_route(&self, route: &str) -> bool {
        self.ignored_routes.contains(route)
            || self.ignored_routes_regex.iter().any(|re| re.is_match(route))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_builder_defaults() {
        let config = Config::builder().api_key("test_key").build().unwrap();

        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.project_id, "");
        assert_eq!(config.api_urls.len(), API_URLS.len());
        assert!(config.masked_fields.contains("password"));
        assert!(config.ignored_routes.contains("/health"));
    }

    #[test]
    fn test_builder_custom_values() {
        let config = Config::builder()
            .api_key("test_key")
            .project_id("test_project")
            .set_api_urls(vec!["https://custom.api"])
            .set_masked_fields(vec!["custom_field"])
            .set_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.project_id, "test_project");
        assert_eq!(config.api_urls, vec!["https://custom.api"]);
        assert!(config.masked_fields.contains("custom_field"));
        assert!(!config.masked_fields.contains("password")); // Default should be gone
        assert!(config.ignored_routes.contains("/custom"));
        assert!(!config.ignored_routes.contains("/health")); // Default should be gone
    }

    #[test]
    fn test_serialization() {
        let original =
            Config::builder().api_key("test_key").project_id("test_project").build().unwrap();

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.api_key, "test_key");
        assert_eq!(deserialized.project_id, "test_project");
        assert_eq!(deserialized.api_urls, original.api_urls);
        assert_eq!(deserialized.masked_fields, original.masked_fields);
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

        let config: Config = serde_json::from_value(json).unwrap();
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.project_id, "test_project");
        assert_eq!(config.api_urls, vec!["https://custom.api"]);
        assert!(config.masked_fields.contains("custom_field"));
        assert!(config.ignored_routes.contains("/custom"));
    }

    #[test]
    fn test_invalid_config() {
        assert!(Config::builder().build().is_err()); // Missing API key
        assert!(Config::builder().api_key("").build().is_err()); // Empty API key
    }

    #[test]
    fn test_add_methods() {
        let config = Config::builder()
            .api_key("test_key")
            .add_api_urls(vec!["https://custom.api"])
            .add_masked_fields(vec!["custom_field"])
            .add_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        // Should have both defaults and custom values
        assert!(config.api_urls.contains(&"https://custom.api".to_string()));
        assert!(config.api_urls.iter().any(|url| url.contains("rocknrolla.treblle.com")));
        assert!(config.masked_fields.contains("custom_field"));
        assert!(config.masked_fields.contains("password"));
        assert!(config.ignored_routes.contains("/custom"));
        assert!(config.ignored_routes.contains("/health"));
    }

    #[test]
    fn test_multiple_adds() {
        let config = Config::builder()
            .api_key("test_key")
            .add_masked_fields(vec!["field1"])
            .add_masked_fields(vec!["field2"])
            .build()
            .unwrap();

        assert!(config.masked_fields.contains("field1"));
        assert!(config.masked_fields.contains("field2"));
        assert!(config.masked_fields.contains("password")); // Default
    }

    #[test]
    fn test_empty_values() {
        let config = Config::builder().api_key("test_key").project_id("").build().unwrap();

        assert_eq!(config.project_id, "");
        assert!(!config.api_urls.is_empty());
    }

    #[test]
    fn test_set_methods() {
        let config = Config::builder()
            .api_key("test_key")
            .set_masked_fields(vec!["custom_field"])
            .set_masked_fields_regex(vec!["custom_.*"])
            .unwrap()
            .set_ignored_routes(vec!["/custom"])
            .build()
            .unwrap();

        // Test that set_masked_fields completely replaces defaults
        assert!(config.should_mask_field("custom_field"));
        assert!(!config.should_mask_field("password")); // Default should be gone
        assert!(!config.should_mask_field("credit_card")); // Default should be gone

        // Test that set_ignored_routes completely replaces defaults
        assert!(config.should_ignore_route("/custom"));
        assert!(!config.should_ignore_route("/health")); // Default should be gone
        assert!(!config.should_ignore_route("/metrics")); // Default should be gone

        // Verify the collections directly
        assert!(config.masked_fields.contains("custom_field"));
        assert!(!config.masked_fields.contains("password"));
        assert_eq!(config.masked_fields.len(), 1);

        assert!(config.ignored_routes.contains("/custom"));
        assert!(!config.ignored_routes.contains("/health"));
        assert_eq!(config.ignored_routes.len(), 1);
    }

    #[test]
    fn test_set_regex_patterns() {
        let config = Config::builder()
            .api_key("test_key")
            .set_masked_fields_regex(vec!["custom_.*"])
            .unwrap()
            .set_ignored_routes_regex(vec!["/test/.*"])
            .unwrap()
            .build()
            .unwrap();

        // Test that set_masked_fields_regex completely replaces defaults
        assert!(config.should_mask_field("custom_field"));
        assert!(!config.should_mask_field("auth_token")); // Default pattern should be gone

        // Test that set_ignored_routes_regex completely replaces defaults
        assert!(config.should_ignore_route("/test/route"));
        assert!(!config.should_ignore_route("/internal/test")); // Default pattern should be gone

        // Verify the collections directly
        assert_eq!(config.masked_fields_regex.len(), 1);
        assert_eq!(config.ignored_routes_regex.len(), 1);
    }
}
