use crate::constants::http::{MAX_POOL_SIZE, MAX_RETRIES};
use crate::{log_debug, log_error};
use serde::Deserialize;
use treblle_core::{Config as CoreConfig, Result, TreblleError};

/// Log level for the WASM host
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    #[default]
    None,
}

impl LogLevel {
    pub fn as_i32(self) -> i32 {
        match self {
            LogLevel::Debug => -1,
            LogLevel::Info => 0,
            LogLevel::Warn => 1,
            LogLevel::Error => 2,
            LogLevel::None => 3,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "none" | _ => LogLevel::None,
        }
    }
}

/// Represents the extended configuration specific to the Traefik WASM middleware
#[derive(Deserialize, Clone, Debug)]
pub struct WasmConfig {
    #[serde(flatten)]
    pub core: CoreConfig,

    /// Controls response buffering for processing
    #[serde(default)]
    pub buffer_response: bool,

    /// Optional path to custom root CA certificate
    #[serde(default)]
    pub root_ca_path: Option<String>,

    /// Maximum number of connection retries (default: 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Maximum size of the connection pool (default: 10)
    #[serde(default = "default_max_pool_size")]
    pub max_pool_size: usize,

    /// Log level for WASM host (default: None)
    #[serde(default)]
    pub log_level: LogLevel,
}

fn default_max_retries() -> usize {
    MAX_RETRIES
}
fn default_max_pool_size() -> usize {
    MAX_POOL_SIZE
}

impl WasmConfig {
    /// Attempts to get the configuration from the host environment.
    /// Falls back to default values if unsuccessful.
    pub fn get_or_fallback() -> Self {
        match Self::get_from_host() {
            Ok(config) => {
                if let Err(e) = config.validate() {
                    log_error!("Invalid configuration: {}", e);
                    Self::default()
                } else {
                    config
                }
            }
            Err(e) => {
                log_error!("Failed to parse config: {}, using fallback", e);
                Self::default()
            }
        }
    }

    /// Retrieves and parses configuration from the host environment
    fn get_from_host() -> Result<Self> {
        let raw_config = crate::host_functions::host_get_config()?;
        let config: WasmConfig = serde_json::from_str(&raw_config)?;

        log_debug!("Received config from host: {:?}", config);

        Ok(config)
    }

    /// Initializes logging based on configuration
    pub fn init_logging(&self) {
        let level = self.log_level.as_i32();
        log_debug!("Setting log level to: {:?}", level);
    }

    /// Validates the configuration
    pub fn validate(&self) -> Result<()> {
        self.core.validate()?;

        // Add any WASM-specific validation here
        if let Some(path) = &self.root_ca_path {
            if path.is_empty() {
                return Err(TreblleError::Config("Root CA path cannot be empty".into()));
            }
        }

        Ok(())
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            buffer_response: false,
            root_ca_path: None,
            max_retries: default_max_retries(),
            max_pool_size: default_max_pool_size(),
            log_level: LogLevel::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_config_deserialize() {
        let json = json!({
            "api_key": "test_key",
            "project_id": "test_project",
            "buffer_response": true,
            "root_ca_path": "/path/to/ca.pem",
            "max_retries": 5,
            "max_pool_size": 20,
            "log_level": "debug"
        });

        let config: WasmConfig = serde_json::from_value(json).unwrap();

        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
        assert_eq!(config.root_ca_path, Some("/path/to/ca.pem".into()));
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.max_pool_size, 20);
        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("Warning"), LogLevel::Warn);
        assert_eq!(LogLevel::from_str("ERROR"), LogLevel::Error);
        assert_eq!(LogLevel::from_str("none"), LogLevel::None);
        assert_eq!(LogLevel::from_str("invalid"), LogLevel::None);
    }
}
