use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::constants::{
    DEFAULT_IGNORED_ROUTES_REGEX, DEFAULT_SENSITIVE_KEYS_REGEX, DEFAULT_TREBLLE_API_URLS,
};
use crate::error::{Result, TreblleError};

/// Configuration for Treblle integrations.
#[derive(Clone, Debug)]
pub struct Config {
    /// The Treblle API key.
    pub api_key: String,
    /// The Treblle project ID.
    pub project_id: String,
    /// The base URLs for the Treblle API.
    pub api_urls: Vec<String>,
    /// Regex patterns for fields to mask
    pub masked_fields: Vec<Regex>,
    /// Regex patterns for routes to ignore
    pub ignored_routes: Vec<Regex>,
}

// Custom serialization for Config to make it debuggable
impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Config", 5)?;
        state.serialize_field("api_key", &self.api_key)?;
        state.serialize_field("project_id", &self.project_id)?;
        state.serialize_field("api_urls", &self.api_urls)?;
        state.serialize_field(
            "masked_fields",
            &self
                .masked_fields
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<&str>>(),
        )?;
        state.serialize_field(
            "ignored_routes",
            &self
                .ignored_routes
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<&str>>(),
        )?;
        state.end()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            api_key: String::new(),
            project_id: String::new(),
            api_urls: default_api_urls(),
            masked_fields: default_masked_fields(),
            ignored_routes: default_ignored_routes(),
        }
    }
}

impl Config {
    /// Create a new Config instance with default patterns
    pub fn new(api_key: String, project_id: String) -> Self {
        Config {
            api_key,
            project_id,
            api_urls: default_api_urls(),
            masked_fields: default_masked_fields(),
            ignored_routes: default_ignored_routes(),
        }
    }

    /// Add additional fields to mask (extends default patterns)
    pub fn add_masked_fields(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p)
                    .map_err(|e| TreblleError::Config(format!("Invalid masked field pattern: {e}")))
            })
            .collect();
        self.masked_fields.extend(new_patterns?);
        Ok(self)
    }

    /// Set masked fields (overrides default patterns)
    pub fn set_masked_fields(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p)
                    .map_err(|e| TreblleError::Config(format!("Invalid masked field pattern: {e}")))
            })
            .collect();
        self.masked_fields = new_patterns?;
        Ok(self)
    }

    /// Add routes to ignore (extends default patterns)
    pub fn add_ignored_routes(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p)
                    .map_err(|e| TreblleError::Config(format!("Invalid route pattern: {e}")))
            })
            .collect();
        self.ignored_routes.extend(new_patterns?);
        Ok(self)
    }

    /// Set ignored routes (overrides default patterns)
    pub fn set_ignored_routes(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        let new_patterns: Result<Vec<Regex>> = patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p)
                    .map_err(|e| TreblleError::Config(format!("Invalid route pattern: {e}")))
            })
            .collect();
        self.ignored_routes = new_patterns?;
        Ok(self)
    }

    /// Set custom API URLs (overrides default URLs)
    pub fn set_api_urls(&mut self, urls: Vec<String>) -> &mut Self {
        self.api_urls = urls;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(TreblleError::Config("API key is required".to_string()));
        }

        if self.project_id.is_empty() {
            return Err(TreblleError::Config("Project ID is required".to_string()));
        }

        if self.api_urls.is_empty() {
            return Err(TreblleError::Config(
                "At least one API URL is required".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a Config instance from a JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct ConfigJson {
            api_key: String,
            project_id: String,
            #[serde(default)]
            api_urls: Vec<String>,
            #[serde(default)]
            masked_fields: Vec<String>,
            #[serde(default)]
            ignored_routes: Vec<String>,
        }

        let json_config: ConfigJson = serde_json::from_str(json)
            .map_err(|e| TreblleError::Config(format!("Invalid JSON configuration: {e}")))?;

        let mut config = Config::new(json_config.api_key, json_config.project_id);

        if !json_config.api_urls.is_empty() {
            config.set_api_urls(json_config.api_urls);
        }

        if !json_config.masked_fields.is_empty() {
            config.set_masked_fields(json_config.masked_fields)?;
        }

        if !json_config.ignored_routes.is_empty() {
            config.set_ignored_routes(json_config.ignored_routes)?;
        }

        config.validate()?;
        Ok(config)
    }
}

fn default_api_urls() -> Vec<String> {
    DEFAULT_TREBLLE_API_URLS
        .iter()
        .map(|&s| s.to_string())
        .collect()
}

fn default_masked_fields() -> Vec<Regex> {
    vec![Regex::new(DEFAULT_SENSITIVE_KEYS_REGEX).expect("Invalid default masked fields regex")]
}

fn default_ignored_routes() -> Vec<Regex> {
    vec![Regex::new(DEFAULT_IGNORED_ROUTES_REGEX).expect("Invalid default ignored routes regex")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_patterns() {
        let config = Config::new("api_key".to_string(), "project_id".to_string());

        // Test default masked fields
        assert_eq!(config.masked_fields.len(), 1);
        let mask_pattern = &config.masked_fields[0];
        assert!(mask_pattern.is_match("password"));
        assert!(mask_pattern.is_match("PASSWORD")); // case insensitive
        assert!(mask_pattern.is_match("credit_score"));

        // Test default ignored routes
        assert_eq!(config.ignored_routes.len(), 1);
        let route_pattern = &config.ignored_routes[0];
        assert!(route_pattern.is_match("/health"));
        assert!(route_pattern.is_match("/HEALTH")); // case insensitive
        assert!(route_pattern.is_match("/metrics"));
    }

    #[test]
    fn test_add_patterns() {
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());

        // Add masked fields
        config
            .add_masked_fields(vec!["custom_secret.*".to_string()])
            .unwrap();
        assert_eq!(config.masked_fields.len(), 2);
        assert!(config
            .masked_fields
            .iter()
            .any(|r| r.is_match("custom_secret_key")));
        assert!(config.masked_fields.iter().any(|r| r.is_match("password"))); // default still works

        // Add ignored routes
        config
            .add_ignored_routes(vec!["/custom/.*".to_string()])
            .unwrap();
        assert_eq!(config.ignored_routes.len(), 2);
        assert!(config
            .ignored_routes
            .iter()
            .any(|r| r.is_match("/custom/route")));
        assert!(config.ignored_routes.iter().any(|r| r.is_match("/health"))); // default still works
    }

    #[test]
    fn test_set_patterns() {
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());

        // Override masked fields
        config
            .set_masked_fields(vec!["custom_secret.*".to_string()])
            .unwrap();
        assert_eq!(config.masked_fields.len(), 1);
        assert!(config.masked_fields[0].is_match("custom_secret_key"));
        assert!(!config.masked_fields[0].is_match("password")); // default no longer works

        // Override ignored routes
        config
            .set_ignored_routes(vec!["/custom/.*".to_string()])
            .unwrap();
        assert_eq!(config.ignored_routes.len(), 1);
        assert!(config.ignored_routes[0].is_match("/custom/route"));
        assert!(!config.ignored_routes[0].is_match("/health")); // default no longer works
    }

    #[test]
    fn test_invalid_patterns() {
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());

        // Invalid masked field pattern
        assert!(config
            .add_masked_fields(vec!["[invalid".to_string()])
            .is_err());

        // Invalid ignored route pattern
        assert!(config
            .add_ignored_routes(vec!["[invalid".to_string()])
            .is_err());
    }

    #[test]
    fn test_json_serialization() {
        let config = Config::new("api_key".to_string(), "project_id".to_string());
        let json = serde_json::to_string(&config).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["api_key"], "api_key");
        assert_eq!(value["project_id"], "project_id");
        assert!(value["masked_fields"].as_array().unwrap().len() > 0);
        assert!(value["ignored_routes"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn test_from_json() {
        let json = json!({
            "api_key": "test_key",
            "project_id": "test_project",
            "api_urls": ["https://custom.treblle.com"],
            "masked_fields": ["custom_secret.*"],
            "ignored_routes": ["/custom/.*"]
        })
        .to_string();

        let config = Config::from_json(&json).unwrap();
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.project_id, "test_project");
        assert_eq!(config.api_urls, vec!["https://custom.treblle.com"]);
        assert!(config.masked_fields[0].is_match("custom_secret_key"));
        assert!(config.ignored_routes[0].is_match("/custom/route"));
    }

    #[test]
    fn test_validation() {
        // Valid config
        let config = Config::new("api_key".to_string(), "project_id".to_string());
        assert!(config.validate().is_ok());

        // Missing API key
        let config = Config::new("".to_string(), "project_id".to_string());
        assert!(config.validate().is_err());

        // Missing project ID
        let config = Config::new("api_key".to_string(), "".to_string());
        assert!(config.validate().is_err());

        // Empty API URLs
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());
        config.api_urls.clear();
        assert!(config.validate().is_err());
    }
}
