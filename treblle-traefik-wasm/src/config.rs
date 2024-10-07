use serde::{Deserialize, Serialize};

use treblle_core::Config as CoreConfig;

use crate::constants::log_level;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WasmConfig {
    #[serde(flatten)]
    pub core: CoreConfig,
    #[serde(default)]
    pub buffer_response: bool,
    pub root_ca_path: Option<String>,
    #[serde(default = "default_log_level")]
    pub log_level: i32,
}

fn default_log_level() -> i32 {
    log_level::INFO
}

impl WasmConfig {
    pub fn new(core: CoreConfig) -> Self {
        WasmConfig {
            core,
            buffer_response: false,
            root_ca_path: None,
            log_level: default_log_level(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, treblle_core::error::TreblleError> {
        let wasm_config: WasmConfig = serde_json::from_str(json)
            .map_err(|e| treblle_core::error::TreblleError::Config(format!("Invalid JSON configuration: {}", e)))?;
        wasm_config.core.validate()?;
        Ok(wasm_config)
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        WasmConfig {
            core: CoreConfig::default(),
            buffer_response: false,
            root_ca_path: None,
            log_level: default_log_level(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert!(!config.buffer_response);
        assert!(config.root_ca_path.is_none());
        assert_eq!(config.log_level, log_level::INFO);
    }

    #[test]
    fn test_wasm_config_from_json() {
        let json = r#"
        {
            "api_key": "test_key",
            "project_id": "test_project",
            "buffer_response": true,
            "root_ca_path": "/path/to/ca",
            "log_level": -1
        }
        "#;
        let config = WasmConfig::from_json(json).unwrap();
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
        assert_eq!(config.root_ca_path, Some("/path/to/ca".to_string()));
        assert_eq!(config.log_level, log_level::DEBUG);
    }
}