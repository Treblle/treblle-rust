//! Constant values used across all Treblle integrations.
//!
pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB

// HTTP-related constants
pub mod http {
    use std::time::Duration;

    pub const HEADER_CONTENT_TYPE: &str = "Content-Type";
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
}

// Default patterns moved to a separate module for clarity
pub mod defaults {
    pub const API_URLS: [&str; 3] = [
        "https://rocknrolla.treblle.com",
        "https://punisher.treblle.com",
        "https://sicario.treblle.com",
    ];

    // Only string patterns that don't include wildcards or partial matches
    pub const DEFAULT_MASKED_FIELDS: [&str; 9] = [
        "password", "pwd", "secret",
        "password_confirmation", "cc",
        "card_number", "ccv", "ssn",
        "credit_score"
    ];

    // Only patterns that require regex (wildcards, case-insensitive, etc.)
    pub const DEFAULT_MASKED_FIELDS_REGEX: &str =
        r"(?i)(user_\w+_password|api_token_\d+|private_key_\w+)";

    // Only string patterns that don't include wildcards or partial matches
    pub const DEFAULT_IGNORED_ROUTES: [&str; 7] = [
        "/health", "/healthz", "/ping",
        "/metrics", "/ready", "/live",
        "/status"
    ];

    // Only patterns that require regex (wildcards, case-insensitive, etc.)
    pub const DEFAULT_IGNORED_ROUTES_REGEX: &str =
        r"(?i)^/(debug|internal|private|test)/.*$";
}
