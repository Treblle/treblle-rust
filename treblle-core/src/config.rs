use std::collections::HashSet;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::constants::defaults::{
    API_URLS, DEFAULT_IGNORED_ROUTES, DEFAULT_IGNORED_ROUTES_REGEX,
    DEFAULT_MASKED_FIELDS, DEFAULT_MASKED_FIELDS_REGEX,
};
use crate::error::{Result, TreblleError};

/// Wrapper for regex to enable serialization and deserialization
#[derive(Clone, Debug)]
pub struct RegexWrapper(pub Regex);

impl Serialize for RegexWrapper {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for RegexWrapper {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Regex::new(&s)
            .map(RegexWrapper)
            .map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for RegexWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

/// Serialization helper module for regex vectors
mod regex_vec_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(regexes: &Vec<Regex>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let patterns: Vec<&str> = regexes.iter().map(|r| r.as_str()).collect();
        patterns.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Vec<Regex>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let patterns: Vec<String> = Vec::deserialize(deserializer)?;
        patterns
            .into_iter()
            .map(|p| Regex::new(&p).map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Configuration for Treblle integrations.
///
/// This struct provides configuration options for Treblle middleware integrations,
/// including API credentials, data masking patterns, and route filtering.
///
/// # Examples
///
/// Basic configuration:
/// ```
/// use treblle_core::Config;
///
/// let config = Config::new("api_key".to_string(), "project_id");
/// ```
///
/// Adding custom masking patterns:
/// ```
/// # use treblle_core::Config;
/// let mut config = Config::new("api_key".to_string(), "project_id");
///
/// // Add exact field matches
/// config.add_masked_fields(vec![
///     "api_token".to_string(),
///     "secret_key".to_string(),
/// ]);
///
/// // Add regex patterns
/// config.add_masked_fields_regex(vec![
///     r"^token_\d+$".to_string(),
///     r"private_.*".to_string(),
/// ]).unwrap();
/// ```
///
/// Route ignoring:
/// ```
/// # use treblle_core::Config;
/// let mut config = Config::new("api_key".to_string(), "project_id");
///
/// // Ignore specific routes
/// config.add_ignored_routes(vec![
///     "/internal/metrics".to_string(),
///     "/health".to_string(),
/// ]);
///
/// // Ignore routes by pattern
/// config.add_ignored_routes_regex(vec![
///     r"/v1/internal/.*".to_string(),
///     r"/debug/.*".to_string(),
/// ]).unwrap();
/// ```
///
/// Loading from configuration:
/// ```
/// # use treblle_core::Config;
/// let config_json = r#"{
///     "api_key": "my_key",
///     "project_id": "my_project",
///     "api_urls": ["https://api.example.com"],
///     "masked_fields": ["api_token"],
///     "masked_fields_regex": ["secret_.*"],
///     "ignored_routes": ["/health"],
///     "ignored_routes_regex": ["/internal/.*"]
/// }"#;
///
/// let config = Config::from_json(config_json).unwrap();
/// ```
///
/// # Data Masking
///
/// There are two ways to specify fields for masking:
///
/// 1. Exact string matches (`masked_fields`):
///    - Case-sensitive, exact matches
///    - Good for known field names
///    - Example: `"password"`, `"credit_card"`
///
/// 2. Regex patterns (`masked_fields_regex`):
///    - Case-insensitive regex matching
///    - Good for pattern matching
///    - Example: `"password_\d+"`, `"secret_.*"`
///
/// Both approaches can be used simultaneously. A field will be masked if it matches
/// either an exact string match or a regex pattern.
///
/// # Route Ignoring
///
/// Similarly, routes can be ignored using two approaches:
///
/// 1. Exact string matches (`ignored_routes`):
///    - Case-sensitive, exact matches
///    - Good for specific endpoints
///    - Example: `"/health"`, `"/metrics"`
///
/// 2. Regex patterns (`ignored_routes_regex`):
///    - Case-insensitive regex matching
///    - Good for pattern matching
///    - Example: `"/api/v\d+/.*"`, `"/internal/.*"`
///
/// Both approaches can be used simultaneously. A route will be ignored if it matches
/// either an exact string match or a regex pattern.
///
/// # Example
///
/// ```rust
/// use treblle_core::Config;
///
/// let mut config = Config::new("my_api_key".to_string(), "my_project");
///
/// // Add exact string matches
/// config.add_masked_fields(vec!["api_key".to_string(), "secret_token".to_string()]);
/// config.add_ignored_routes(vec!["/health".to_string(), "/metrics".to_string()]);
///
/// // Add regex patterns
/// config.add_masked_fields_regex(vec![
///     r"password_\d+".to_string(),
///     r"secret_.*".to_string()
/// ]).unwrap();
///
/// config.add_ignored_routes_regex(vec![
///     r"/api/v\d+/.*".to_string(),
///     r"/internal/.*".to_string()
/// ]).unwrap();
/// ```
/// # Default Values
///
/// ## Masked Fields
///
/// Default exact matches:
/// ```text
/// password, pwd, secret, password_confirmation, cc,
/// card_number, ccv, ssn, credit_score
/// ```
///
/// Default regex pattern:
/// ```text
/// (?i)(user_\w+_password|api_token_\d+|private_key_\w+)
/// ```
///
/// ## Ignored Routes
///
/// Default exact matches:
/// ```text
/// /health, /healthz, /ping, /metrics, /ready, /live, /status
/// ```
///
/// Default regex pattern:
/// ```text
/// (?i)^/(debug|internal|private|test)/.*$
/// ```
///
/// These defaults are used when no custom patterns are provided. When using `set_*`
/// methods, only the specific category (regex OR exact matches) is replaced while
/// preserving the defaults for the other category.
///
/// # Examples
/// ```
/// use treblle_core::Config;
///
/// let mut config = Config::new("api_key".to_string(), "project_id");
///
/// // Override exact matches (preserves default regex patterns)
/// config.set_masked_fields(vec![
///     "api_token".to_string(),
///     "secret_key".to_string(),
/// ]);
///
/// // Override regex patterns (preserves default exact matches)
/// config.set_masked_fields_regex(vec![
///     r"^token_\d+$".to_string(),
///     r"private_.*".to_string(),
/// ]).unwrap();
///
/// // Add to existing patterns
/// config.add_masked_fields(vec!["custom_field".to_string()]);
/// config.add_masked_fields_regex(vec![r"custom_.*".to_string()]).unwrap();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// The Treblle API key (required).
    pub api_key: String,

    /// The Treblle project ID (optional, defaults to empty string).
    #[serde(default)]
    pub project_id: String,

    /// The base URLs for the Treblle API.
    pub api_urls: Vec<String>,

    /// Regex patterns for fields to mask.
    /// These patterns are applied case-insensitively to field names in request/response data.
    #[serde(with = "regex_vec_serde")]
    pub masked_fields_regex: Vec<Regex>,

    /// Exact string matches for fields to mask.
    /// These are applied as exact, case-sensitive matches to field names.
    #[serde(default)]
    pub masked_fields: HashSet<String>,

    /// Regex patterns for routes to ignore.
    /// These patterns are applied to request paths to determine if they should be processed.
    #[serde(with = "regex_vec_serde")]
    pub ignored_routes_regex: Vec<Regex>,

    /// Exact string matches for routes to ignore.
    /// These are applied as exact, case-sensitive matches to request paths.
    #[serde(default)]
    pub ignored_routes: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            project_id: String::new(),
            api_urls: API_URLS.iter().map(ToString::to_string).collect(),
            masked_fields_regex: vec![Regex::new(DEFAULT_MASKED_FIELDS_REGEX)
                .expect("Invalid default masked fields regex")],
            masked_fields: DEFAULT_MASKED_FIELDS.iter().map(ToString::to_string).collect(),
            ignored_routes_regex: vec![Regex::new(DEFAULT_IGNORED_ROUTES_REGEX)
                .expect("Invalid default ignored routes regex")],
            ignored_routes: DEFAULT_IGNORED_ROUTES.iter().map(ToString::to_string).collect(),
        }
    }
}

impl Config {
    /// Creates a new Config instance with the specified API key and project ID.
    ///
    /// The project ID is optional and can be empty. All other fields are initialized
    /// with default values.
    pub fn new(api_key: String, project_id: impl Into<String>) -> Self {
        Self {
            api_key,
            project_id: project_id.into(),
            ..Default::default()
        }
    }

    /// Helper function to compile regex patterns safely
    fn compile_patterns(patterns: Vec<String>) -> Result<Vec<Regex>> {
        patterns
            .into_iter()
            .map(|p| {
                Regex::new(&p).map_err(|e| {
                    TreblleError::Config(format!("Invalid regex pattern: {e}"))
                })
            })
            .collect()
    }

    /// Adds exact string matches for fields to mask, extending the existing set.
    ///
    /// These patterns are matched exactly and case-sensitively against field names
    /// in request and response data. Both exact matches and regex patterns will be
    /// used for masking.
    ///
    /// Note: When no custom patterns are provided, default values are used for both
    /// exact matches and regex patterns.
    pub fn add_masked_fields(&mut self, fields: impl IntoIterator<Item = String>) -> &mut Self {
        self.masked_fields.extend(fields);
        self
    }

    /// Adds regex patterns for fields to mask, extending the existing patterns.
    ///
    /// These patterns are applied case-insensitively to field names in request
    /// and response data. Both exact matches and regex patterns will be used for
    /// masking.
    ///
    /// Note: When no custom patterns are provided, default values are used for both
    /// exact matches and regex patterns.
    pub fn add_masked_fields_regex(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        self.masked_fields_regex.extend(Self::compile_patterns(patterns)?);
        Ok(self)
    }


    /// Sets the exact string matches for fields to mask, replacing existing string matches.
    ///
    /// Note: This only replaces the exact string matches; regex patterns remain unchanged.
    /// When no custom patterns are provided for either category, default values are used.
    ///
    /// # Default Behavior
    /// - Only replaces exact string matches, preserving regex patterns
    /// - If this is the only customization, default regex patterns will still apply
    pub fn set_masked_fields(&mut self, fields: impl IntoIterator<Item = String>) -> &mut Self {
        self.masked_fields = fields.into_iter().collect();
        self
    }

    /// Sets the regex patterns for fields to mask, replacing existing regex patterns.
    ///
    /// Note: This only replaces the regex patterns; exact string matches remain unchanged.
    /// When no custom patterns are provided for either category, default values are used.
    ///
    /// # Default Behavior
    /// - Only replaces regex patterns, preserving exact string matches
    /// - If this is the only customization, default exact matches will still apply
    pub fn set_masked_fields_regex(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        self.masked_fields_regex = Self::compile_patterns(patterns)?;
        Ok(self)
    }


    /// Adds exact string matches for routes to ignore, extending the existing set.
    ///
    /// These routes will be skipped by the middleware and not sent to Treblle.
    /// Both exact matches and regex patterns will be used for route ignoring.
    ///
    /// Note: When no custom patterns are provided, default values are used for both
    /// exact matches and regex patterns.
    pub fn add_ignored_routes(&mut self, routes: impl IntoIterator<Item = String>) -> &mut Self {
        self.ignored_routes.extend(routes);
        self
    }

    /// Adds regex patterns for routes to ignore, extending the existing patterns.
    ///
    /// These patterns are applied to request paths to determine if they should be skipped.
    /// Both exact matches and regex patterns will be used for route ignoring.
    ///
    /// Note: When no custom patterns are provided, default values are used for both
    /// exact matches and regex patterns.
    pub fn add_ignored_routes_regex(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        self.ignored_routes_regex.extend(Self::compile_patterns(patterns)?);
        Ok(self)
    }

    /// Sets the exact string matches for routes to ignore, replacing existing string matches.
    ///
    /// Note: This only replaces the exact string matches; regex patterns remain unchanged.
    /// When no custom patterns are provided for either category, default values are used.
    ///
    /// # Default Behavior
    /// - Only replaces exact string matches, preserving regex patterns
    /// - If this is the only customization, default regex patterns will still apply
    pub fn set_ignored_routes(&mut self, routes: impl IntoIterator<Item = String>) -> &mut Self {
        self.ignored_routes = routes.into_iter().collect();
        self
    }

    /// Sets the regex patterns for routes to ignore, replacing existing regex patterns.
    ///
    /// Note: This only replaces the regex patterns; exact string matches remain unchanged.
    /// When no custom patterns are provided for either category, default values are used.
    ///
    /// # Default Behavior
    /// - Only replaces regex patterns, preserving exact string matches
    /// - If this is the only customization, default exact matches will still apply
    pub fn set_ignored_routes_regex(&mut self, patterns: Vec<String>) -> Result<&mut Self> {
        self.ignored_routes_regex = Self::compile_patterns(patterns)?;
        Ok(self)
    }

    /// Sets the API URLs to use for Treblle API communication.
    ///
    /// This replaces the default URLs with the provided set. At least one URL
    /// must be provided.
    pub fn set_api_urls(&mut self, urls: Vec<String>) -> &mut Self {
        self.api_urls = urls;
        self
    }

    /// Checks if a field should be masked based on default or custom patterns.
    ///
    /// A field will be masked if it matches either:
    /// - Any regex pattern (default or custom)
    /// - Any exact string match (default or custom)
    ///
    /// If no custom patterns are provided for either category, default values are used.
    pub fn should_mask_field(&self, field: &str) -> bool {
        self.masked_fields.contains(field) ||
            self.masked_fields_regex.iter().any(|re| re.is_match(field))
    }

    /// Checks if a route should be ignored based on default or custom patterns.
    ///
    /// A route will be ignored if it matches either:
    /// - Any regex pattern (default or custom)
    /// - Any exact string match (default or custom)
    ///
    /// If no custom patterns are provided for either category, default values are used.
    pub fn should_ignore_route(&self, route: &str) -> bool {
        self.ignored_routes.contains(route) ||
            self.ignored_routes_regex.iter().any(|re| re.is_match(route))
    }

    /// Validates the configuration.
    ///
    /// Ensures that:
    /// - API key is not empty
    /// - At least one API URL is configured
    ///
    /// Note that project_id is optional and can be empty.
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(TreblleError::Config("API key is required".to_string()));
        }

        if self.api_urls.is_empty() {
            return Err(TreblleError::Config(
                "At least one API URL is required".to_string(),
            ));
        }

        Ok(())
    }

    /// Creates a Config instance from a JSON string.
    ///
    /// This is useful for loading configuration from files or environment variables.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| TreblleError::Config(format!("Invalid JSON configuration: {e}")))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    mod defaults {
        use super::*;

        #[test]
        fn test_default_config() {
            let config = Config::default();
            assert!(config.api_key.is_empty());
            assert!(config.project_id.is_empty());
            assert!(!config.api_urls.is_empty());

            // Test default masked fields (exact matches)
            assert!(config.masked_fields.contains("password"));
            assert!(config.masked_fields.contains("secret"));
            assert!(config.masked_fields.contains("api_key"));
            assert!(config.masked_fields.contains("card_number"));
            assert!(config.masked_fields.contains("ssn"));

            // Test default masked regex
            assert!(config.should_mask_field("password_hash"));
            assert!(config.should_mask_field("auth_token"));
            assert!(config.should_mask_field("access_token_secret"));
            assert!(config.should_mask_field("card_pin"));

            // Test default ignored routes (exact matches)
            assert!(config.ignored_routes.contains("/health"));
            assert!(config.ignored_routes.contains("/metrics"));
            assert!(config.ignored_routes.contains("/_debug"));

            // Test default ignored regex
            assert!(config.should_ignore_route("/debug/test"));
            assert!(config.should_ignore_route("/internal/metrics"));
            assert!(config.should_ignore_route("/admin/users"));
            assert!(config.should_ignore_route("/swagger/api"));
        }

        #[test]
        fn test_new_config() {
            let config = Config::new("test_key".to_string(), "test_project");
            assert_eq!(config.api_key, "test_key");
            assert_eq!(config.project_id, "test_project");

            // Empty project_id should be valid
            let config = Config::new("test_key".to_string(), "");
            assert_eq!(config.project_id, "");
        }
    }

    mod ignored_routes {
        use super::*;

        #[test]
        fn test_add_ignored_routes() {
            let mut config = Config::default();
            let initial_size = config.ignored_routes.len(); // 12 from our new defaults

            config.add_ignored_routes(vec!["/custom/route".to_string(), "/test/api".to_string()]);

            assert!(config.ignored_routes.contains("/custom/route"));
            assert!(config.ignored_routes.contains("/test/api"));
            assert_eq!(config.ignored_routes.len(), initial_size + 2);

            // Original defaults should still work
            assert!(config.should_ignore_route("/health"));
            assert!(config.should_ignore_route("/metrics"));
        }

        #[test]
        fn test_set_ignored_routes() {
            let mut config = Config::default();

            config.set_ignored_routes(vec!["/api/internal".to_string(), "/debug".to_string()]);

            // New routes should work
            assert!(config.ignored_routes.contains("/api/internal"));
            assert!(config.ignored_routes.contains("/debug"));

            // Default exact matches should be gone
            assert!(!config.ignored_routes.contains("/health"));

            // Regex patterns should still work for their patterns
            assert!(config.should_ignore_route("/internal/test"));
        }

        #[test]
        fn test_add_ignored_routes_regex() {
            let mut config = Config::default();
            let original_count = config.ignored_routes_regex.len();

            config.add_ignored_routes_regex(vec![
                r"/api/v\d+/.*".to_string(),
                r"/internal/.*".to_string()
            ]).unwrap();

            assert!(config.should_ignore_route("/api/v1/users"));
            assert!(config.should_ignore_route("/internal/debug"));
            assert_eq!(config.ignored_routes_regex.len(), original_count + 2);

            // Should still match default string patterns
            assert!(config.ignored_routes.contains("/health"));
        }

        #[test]
        fn test_set_ignored_routes_regex() {
            let mut config = Config::default();

            config.set_ignored_routes_regex(vec![
                r"/api/v\d+/.*".to_string(),
                r"/admin/.*".to_string()
            ]).unwrap();

            // New patterns should work
            assert!(config.should_ignore_route("/api/v1/users"));
            assert!(config.should_ignore_route("/admin/dashboard"));

            // Default exact matches should still work
            assert!(config.ignored_routes.contains("/health"));

            // Old regex patterns should not work
            assert!(!config.should_ignore_route("/debug/test"));
        }
    }

    #[test]
    fn test_overlapping_patterns() {
        let mut config = Config::default();

        // Add patterns that could match the same fields
        config.add_masked_fields(vec!["password".to_string()]);
        config.add_masked_fields_regex(vec![r"(?i)password.*".to_string()]).unwrap();

        // Both types should work independently
        assert!(config.should_mask_field("password")); // Exact match
        assert!(config.should_mask_field("password123")); // Regex match

        // Setting new regex patterns shouldn't affect string matches
        config.set_masked_fields_regex(vec![r"secret_.*".to_string()]).unwrap();

        assert!(config.should_mask_field("password")); // Original exact match still works
        assert!(config.should_mask_field("secret_key")); // New regex pattern works
        assert!(!config.should_mask_field("password123")); // Old regex pattern is gone

        // Same for routes
        config.add_ignored_routes(vec!["/health".to_string()]);
        config.add_ignored_routes_regex(vec![r"/health/.*".to_string()]).unwrap();

        assert!(config.should_ignore_route("/health")); // Exact match
        assert!(config.should_ignore_route("/health/check")); // Regex match
    }

    mod masked_fields {
        use super::*;

        #[test]
        fn test_add_masked_fields() {
            let mut config = Config::default();
            let initial_size = config.masked_fields.len(); // 15 from our new defaults

            config.add_masked_fields(vec!["custom_key".to_string(), "token_test".to_string()]);

            assert!(config.masked_fields.contains("custom_key"));
            assert!(config.masked_fields.contains("token_test"));
            assert_eq!(config.masked_fields.len(), initial_size + 2);

            // Original defaults should still work
            assert!(config.should_mask_field("password"));
            assert!(config.should_mask_field("api_key"));
        }

        #[test]
        fn test_set_masked_fields() {
            let mut config = Config::default();

            config.set_masked_fields(vec!["api_key".to_string(), "token".to_string()]);

            assert_eq!(config.masked_fields.len(), 2);
            assert!(config.masked_fields.contains("api_key"));
            assert!(config.masked_fields.contains("token"));

            // Default string patterns should be gone
            assert!(!config.masked_fields.contains("password"));
            // Regex patterns should still work for other sensitive data
            assert!(config.should_mask_field("auth_token_secret"));
        }

        #[test]
        fn test_add_masked_fields_regex() {
            let mut config = Config::default();
            let original_count = config.masked_fields_regex.len();

            config.add_masked_fields_regex(vec![
                r"secret_\d+".to_string(),
                r"private_.*".to_string()
            ]).unwrap();

            assert!(config.should_mask_field("secret_123"));
            assert!(config.should_mask_field("private_key"));
            assert_eq!(config.masked_fields_regex.len(), original_count + 2);

            // Should still match default string patterns
            assert!(config.masked_fields.contains("password"));
        }

        #[test]
        fn test_set_masked_fields_regex() {
            let mut config = Config::default();

            config.set_masked_fields_regex(vec![
                r"secret_\d+".to_string(),
                r"private_.*".to_string()
            ]).unwrap();

            assert_eq!(config.masked_fields_regex.len(), 2);
            assert!(config.should_mask_field("secret_123"));
            assert!(config.should_mask_field("private_key"));

            // Should still match default string patterns
            assert!(config.masked_fields.contains("password"));
            // Original regex patterns should be gone
            assert!(!config.should_mask_field("auth_token"));
        }
    }

    mod api_urls {
        use super::*;

        #[test]
        fn test_set_api_urls() {
            let mut config = Config::default();
            let new_urls = vec!["https://api1.example.com".to_string(), "https://api2.example.com".to_string()];

            config.set_api_urls(new_urls.clone());

            assert_eq!(config.api_urls, new_urls);
        }
    }

    mod serialization {
        use super::*;

        #[test]
        fn test_config_serialization() {
            let original = Config::new("test_key".to_string(), "test_project");
            let serialized = serde_json::to_string(&original).unwrap();
            let deserialized: Config = serde_json::from_str(&serialized).unwrap();

            assert_eq!(deserialized.api_key, original.api_key);
            assert_eq!(deserialized.project_id, original.project_id);
            assert_eq!(deserialized.api_urls, original.api_urls);
            assert_eq!(deserialized.masked_fields, original.masked_fields);
            assert_eq!(
                deserialized.masked_fields_regex[0].as_str(),
                original.masked_fields_regex[0].as_str()
            );
            assert_eq!(
                deserialized.ignored_routes_regex[0].as_str(),
                original.ignored_routes_regex[0].as_str()
            );
        }

        #[test]
        fn test_json_conversion() {
            let json = json!({
                "api_key": "test_key",
                "project_id": "test_project",
                "api_urls": ["https://api.example.com"],
                "masked_fields": ["custom_secret"],
                "masked_fields_regex": ["secret_.*"],
                "ignored_routes": ["/custom"],
                "ignored_routes_regex": ["/custom/.*"]
            }).to_string();

            let config = Config::from_json(&json).unwrap();
            assert_eq!(config.api_key, "test_key");
            assert_eq!(config.project_id, "test_project");
            assert_eq!(config.api_urls, vec!["https://api.example.com"]);
            assert!(config.masked_fields.contains("custom_secret"));
            assert!(config.should_mask_field("secret_key"));
            assert!(config.ignored_routes.contains("/custom"));
            assert!(config.should_ignore_route("/custom/route"));
        }

        #[test]
        fn test_invalid_json() {
            let result = Config::from_json("invalid json");
            assert!(result.is_err());
        }
    }

    mod validation {
        use super::*;

        #[test]
        fn test_validation_requirements() {
            // Valid config
            let config = Config::new("api_key".to_string(), "project_id");
            assert!(config.validate().is_ok());

            // Missing API key
            let config = Config::new("".to_string(), "project_id");
            assert!(config.validate().is_err());

            // Empty project ID is valid
            let config = Config::new("api_key".to_string(), "");
            assert!(config.validate().is_ok());

            // No API URLs
            let mut config = Config::new("api_key".to_string(), "project_id");
            config.api_urls.clear();
            assert!(config.validate().is_err());
        }
    }
}
