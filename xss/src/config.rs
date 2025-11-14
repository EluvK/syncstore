use serde::Deserialize;
use ss_utils::logs::LogConfig;
use syncstore::config::{ServiceConfig, StoreConfig};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_config: LogConfig,
    pub service_config: ServiceConfig,
    pub store_config: StoreConfig,
}

impl Config {
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
