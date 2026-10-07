use eld_client::config::ClientConfig;
use eld_client::endpoint::resolve_node_base_url;
use eld_common::error::EldError;
use serde::Deserialize;
use std::fs;

pub const DEFAULT_FAUCET_CONFIG_PATH: &str = "config/faucet_config.json";

/// Tendermint RPC port.
pub const NODE_PORT: &str = "26657";
/// Placeholder app REST port for [`ClientConfig`] (unused by the faucet binary).
pub const APP_PORT: &str = "9001";
/// Bind address for the HTTP server (all interfaces, including Docker).
pub const BIND_HOST: &str = "0.0.0.0";
/// Listen port for the HTTP server.
pub const BIND_PORT: &str = "8080";
/// GET health check path.
pub const HEALTH_PATH: &str = "/health";
/// POST path for faucet requests (matches `eld-cli`).
pub const REQUEST_PATH: &str = "/faucet/request";

/// Fields the faucet binary reads from disk.
#[derive(Debug, Clone, Deserialize)]
pub struct FaucetConfig {
    pub node_host: String,
    pub chain_id: String,
}

impl FaucetConfig {
    pub fn load() -> Result<Self, EldError> {
        Self::from_file(DEFAULT_FAUCET_CONFIG_PATH)
    }

    pub fn from_file(path: &str) -> Result<Self, EldError> {
        let data = fs::read_to_string(path).map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Failed to read configuration file '{path}': {e}"),
        })?;

        let config: Self = serde_json::from_str(&data).map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Failed to parse configuration JSON from '{path}': {e}"),
        })?;

        config.validate().map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Configuration validation failed for '{path}': {e}"),
        })?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<(), EldError> {
        resolve_node_base_url(None, &self.node_host, NODE_PORT)?;
        if self.chain_id.trim().is_empty() {
            return EldError::validation_error("chain_id", "empty", "Chain ID cannot be empty");
        }
        Ok(())
    }

    pub fn node_url(&self) -> Result<String, EldError> {
        resolve_node_base_url(None, &self.node_host, NODE_PORT)
    }

    pub fn bind_addr() -> String {
        format!("{BIND_HOST}:{BIND_PORT}")
    }

    /// Build a [`ClientConfig`] for [`eld_client::ChainClient`]. Unused client fields
    /// get placeholders so the type is complete.
    pub fn to_client_config(&self) -> ClientConfig {
        ClientConfig {
            node_host: self.node_host.clone(),
            node_port: NODE_PORT.to_string(),
            faucet_host: BIND_HOST.to_string(),
            faucet_port: BIND_PORT.to_string(),
            faucet_end_point: REQUEST_PATH.to_string(),
            faucet_url: None,
            app_port: APP_PORT.to_string(),
            node_url: None,
            app_url: None,
            chain_id: self.chain_id.clone(),
        }
    }
}
