//! Core functionality for Treblle Rust integrations.
//!
//! This crate provides shared components and utilities for Treblle integrations
//! across different Rust web frameworks and environments.

pub mod config;
pub mod constants;
pub mod error;
pub mod extractors;
pub mod payload;
pub mod schema;
pub mod utils;

#[cfg(feature = "http_client")]
pub mod http_client;

#[cfg(feature = "http_client")]
pub use http_client::TreblleClient;

pub use config::{Config, ConfigBuilder};
pub use error::{Result, TreblleError};
pub use payload::PayloadBuilder;
pub use schema::{ErrorInfo, LanguageInfo, RequestInfo, ResponseInfo, ServerInfo};

pub use utils::mask_sensitive_data;

/// The version of the Treblle SDK.
pub const TREBLLE_SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treblle_sdk_version() {
        assert!(!TREBLLE_SDK_VERSION.is_empty());
    }
}
