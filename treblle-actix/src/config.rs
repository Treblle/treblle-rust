use serde::{Deserialize, Serialize};

use treblle_core::Config;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActixConfig {
    #[serde(flatten)]
    pub core: Config,
    // Add any Actix-specific configuration options here
    // For example:
    #[serde(default)]
    pub buffer_response: bool,
}

impl ActixConfig {
    pub fn new(api_key: String, project_id: String) -> Self {
        ActixConfig {
            core: Config::new(api_key, project_id),
            buffer_response: false,
        }
    }

    pub fn buffer_response(mut self, buffer: bool) -> Self {
        self.buffer_response = buffer;
        self
    }
}

impl Default for ActixConfig {
    fn default() -> Self {
        ActixConfig {
            core: Config::default(),
            buffer_response: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actix_config() {
        let config = ActixConfig::new("test_key".to_string(), "test_project".to_string())
            .buffer_response(true);
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
        assert!(config.buffer_response);
    }
}