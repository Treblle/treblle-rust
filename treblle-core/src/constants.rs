//! Constant values used across all Treblle integrations.

/// Treblle API URLs
pub const DEFAULT_TREBLLE_API_URLS: [&str; 3] = [
    "https://rocknrolla.treblle.com",
    "https://punisher.treblle.com",
    "https://sicario.treblle.com",
];

/// Default regex pattern for sensitive keys
pub const DEFAULT_SENSITIVE_KEYS_REGEX: &str =
    r"(?i)(password|pwd|secret|password_confirmation|cc|card_number|ccv|ssn|credit_score)";

/// Default regex pattern for ignored routes (health checks, metrics, etc.)
pub const DEFAULT_IGNORED_ROUTES_REGEX: &str =
    r"(?i)^/(health|healthz|ping|metrics|ready|live|alive|status)/?$";

/// HTTP-related constants
pub mod http {
    pub const HEADER_CONTENT_TYPE: &str = "Content-Type";
    pub const TIMEOUT_SECONDS: u64 = 10;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_TREBLLE_API_URLS.len(), 3);
        assert!(!DEFAULT_SENSITIVE_KEYS_REGEX.is_empty());
        assert_eq!(http::HEADER_CONTENT_TYPE, "Content-Type");
    }
}
