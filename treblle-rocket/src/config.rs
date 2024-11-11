use serde::{Deserialize, Serialize};
use treblle_core::Config as CoreConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RocketConfig {
    #[serde(flatten)]
    pub core: CoreConfig,
    // Add any Rocket-specific configuration options here if needed
}

impl RocketConfig {
    pub fn new(api_key: String, project_id: String) -> Self {
        RocketConfig { core: CoreConfig::new(api_key, project_id) }
    }

    pub fn add_masked_fields(&mut self, fields: Vec<String>) -> &mut Self {
        self.core.add_masked_fields(fields);
        self
    }

    pub fn add_ignored_routes(&mut self, routes: Vec<String>) -> &mut Self {
        self.core.add_ignored_routes(routes);
        self
    }
}

impl Default for RocketConfig {
    fn default() -> Self {
        RocketConfig { core: CoreConfig::default() }
    }
}
