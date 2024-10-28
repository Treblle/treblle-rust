use serde::{Deserialize, Serialize};
use treblle_core::Config as CoreConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RocketConfig {
    #[serde(flatten)]
    pub core: CoreConfig,
}

impl RocketConfig {
    pub fn new(api_key: String, project_id: String) -> Self {
        RocketConfig {
            core: CoreConfig::new(api_key, project_id),
        }
    }
}

impl Default for RocketConfig {
    fn default() -> Self {
        RocketConfig {
            core: CoreConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rocket_config() {
        let config = RocketConfig::new("test_key".to_string(), "test_project".to_string());
        assert_eq!(config.core.api_key, "test_key");
        assert_eq!(config.core.project_id, "test_project");
    }
}
