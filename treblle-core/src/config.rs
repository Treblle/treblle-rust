use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Result, TreblleError};
use crate::constants::{DEFAULT_TREBLLE_API_URLS, DEFAULT_SENSITIVE_KEYS_REGEX};


/// Configuration for Treblle integrations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// The Treblle API key.
    pub api_key: String,
    /// The Treblle project ID.
    pub project_id: String,
    /// The base URLs for the Treblle API.
    #[serde(default = "default_api_urls")]
    pub api_urls: Vec<String>,
    /// Fields to mask in the request and response payloads.
    #[serde(default = "default_masked_fields")]
    pub masked_fields: Vec<String>,
    /// Routes to ignore when sending data to Treblle.
    #[serde(default)]
    pub ignored_routes: Vec<Regex>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            api_key: String::new(),
            project_id: String::new(),
            api_urls: default_api_urls(),
            masked_fields: default_masked_fields(),
            ignored_routes: Vec::new(),
        }
    }
}

fn default_api_urls() -> Vec<String> {
    DEFAULT_TREBLLE_API_URLS.iter().map(|&s| s.to_string()).collect()
}

fn default_masked_fields() -> Vec<String> {
    DEFAULT_SENSITIVE_KEYS_REGEX
        .trim_start_matches("(?i)(")
        .trim_end_matches(')')
        .split('|')
        .map(|s| s.to_string())
        .collect()
}

impl Config {
    /// Create a new Config instance.
    pub fn new(api_key: String, project_id: String) -> Self {
        Config {
            api_key,
            project_id,
            api_urls: default_api_urls(),
            masked_fields: default_masked_fields(),
            ignored_routes: Vec::new(),
        }
    }

    /// Add additional fields to mask.
    pub fn add_masked_fields(&mut self, fields: Vec<String>) -> &mut Self {
        self.masked_fields.extend(fields);
        self
    }

    /// Add routes to ignore.
    pub fn add_ignored_routes(&mut self, routes: Vec<String>) -> &mut Self {
        self.ignored_routes.extend(routes.into_iter().map(|r| Regex::new(&r).expect("Invalid regex")));
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(TreblleError::Config("API key is required".to_string()));
        }
        if self.project_id.is_empty() {
            return Err(TreblleError::Config("Project ID is required".to_string()));
        }
        Ok(())
    }

    /// Create a Config instance from a JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        let config: Config = serde_json::from_str(json)
            .map_err(|e| TreblleError::Config(format!("Invalid JSON configuration: {e}")))?;
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = Config::new("api_key".to_string(), "project_id".to_string());
        assert_eq!(config.api_key, "api_key");
        assert_eq!(config.project_id, "project_id");
        assert_eq!(config.api_urls, default_api_urls());
        assert_eq!(config.masked_fields, default_masked_fields());
        assert!(config.ignored_routes.is_empty());
    }

    #[test]
    fn test_config_add_masked_fields() {
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());
        config.add_masked_fields(vec!["custom_field".to_string()]);
        assert!(config.masked_fields.contains(&"custom_field".to_string()));
    }

    #[test]
    fn test_config_add_ignored_routes() {
        let mut config = Config::new("api_key".to_string(), "project_id".to_string());
        config.add_ignored_routes(vec!["/health".to_string()]);
        assert!(config.ignored_routes.iter().any(|r| r.as_str() == "/health"));
    }

    #[test]
    fn test_config_validation() {
        let valid_config = Config::new("api_key".to_string(), "project_id".to_string());
        assert!(valid_config.validate().is_ok());

        let invalid_config = Config {
            api_key: String::new(),
            project_id: "project_id".to_string(),
            ..Config::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_from_json() {
        let json = r#"
        {
            "api_key": "test_key",
            "project_id": "test_project",
            "api_urls": ["https://custom.treblle.com"],
            "masked_fields": ["custom_field"],
            "ignored_routes": ["/test"]
        }
        "#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.project_id, "test_project");
        assert_eq!(config.api_urls, vec!["https://custom.treblle.com"]);
        assert_eq!(config.masked_fields, vec!["custom_field"]);
        assert!(config.ignored_routes.iter().any(|r| r.as_str() == "/test"));
    }
}