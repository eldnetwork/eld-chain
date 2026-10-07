use eld_client::config::{ClientConfig, WALLETS_PATH};
use eld_client::endpoint::resolve_node_base_url;
use eld_common::error::EldError;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_FAUCET_CONFIG_PATH: &str = "config/faucet_config.json";

/// Tendermint RPC port.
pub const NODE_PORT: &str = "26657";
/// Placeholder app REST port for [`ClientConfig`] (unused by the faucet binary).
pub const APP_PORT: &str = "9001";
/// GET health check path.
pub const HEALTH_PATH: &str = "/health";
/// POST path for faucet requests (matches `eld-cli`).
pub const REQUEST_PATH: &str = "/faucet/request";

fn default_bind_host() -> String {
    "0.0.0.0".to_string()
}

fn default_bind_port() -> u16 {
    8080
}

fn default_drip() -> u64 {
    1_000_000_000
}

fn default_address_daily() -> u32 {
    1
}

fn default_ip_hourly() -> u32 {
    3
}

fn default_reserve() -> u64 {
    10 * default_drip()
}

fn default_db_path() -> String {
    "data/faucet.db".to_string()
}

fn default_wallet_name() -> String {
    "wallet-faucet-1".to_string()
}

/// Fields the faucet binary reads from disk (plus env overrides).
#[derive(Debug, Clone, Deserialize)]
pub struct FaucetConfig {
    pub node_host: String,
    pub chain_id: String,
    #[serde(default = "default_bind_host")]
    pub bind_host: String,
    #[serde(default = "default_bind_port")]
    pub bind_port: u16,
    #[serde(default = "default_drip")]
    pub drip_base_units: u64,
    #[serde(default = "default_address_daily")]
    pub address_daily_drips: u32,
    #[serde(default = "default_ip_hourly")]
    pub ip_hourly_requests: u32,
    /// Wired by the hot-wallet reserve check (not yet used at request time).
    #[allow(dead_code)]
    #[serde(default = "default_reserve")]
    pub hot_wallet_reserve: u64,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_wallet_name")]
    pub wallet_name: String,
    /// Resolved wallet file path (default `wallets/wallets.json`, or `FAUCET_WALLET_PATH`).
    #[serde(skip)]
    pub wallet_path: PathBuf,
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

        let mut config: Self = serde_json::from_str(&data).map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Failed to parse configuration JSON from '{path}': {e}"),
        })?;
        config.wallet_path = PathBuf::from(WALLETS_PATH);
        config
            .apply_env_overrides()
            .map_err(|e| EldError::ConfigError {
                file: path.to_string(),
                details: e,
            })?;

        config.validate().map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Configuration validation failed for '{path}': {e}"),
        })?;

        Ok(config)
    }

    fn apply_env_overrides(&mut self) -> Result<(), String> {
        if let Ok(host) = env::var("FAUCET_BIND_HOST") {
            if host.trim().is_empty() {
                return Err("FAUCET_BIND_HOST is empty".to_string());
            }
            self.bind_host = host;
        }
        if let Ok(port) = env::var("FAUCET_BIND_PORT") {
            self.bind_port = port
                .parse()
                .map_err(|_| format!("FAUCET_BIND_PORT is not a valid u16: {port}"))?;
        }
        if let Ok(db_path) = env::var("FAUCET_DB_PATH") {
            if db_path.trim().is_empty() {
                return Err("FAUCET_DB_PATH is empty".to_string());
            }
            self.db_path = db_path;
        }
        if let Ok(wallet_path) = env::var("FAUCET_WALLET_PATH") {
            if wallet_path.trim().is_empty() {
                return Err("FAUCET_WALLET_PATH is empty".to_string());
            }
            self.wallet_path = PathBuf::from(wallet_path);
        }
        if let Ok(wallet_name) = env::var("FAUCET_WALLET_NAME") {
            if wallet_name.trim().is_empty() {
                return Err("FAUCET_WALLET_NAME is empty".to_string());
            }
            self.wallet_name = wallet_name;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), EldError> {
        resolve_node_base_url(None, &self.node_host, NODE_PORT)?;
        if self.chain_id.trim().is_empty() {
            return EldError::validation_error("chain_id", "empty", "Chain ID cannot be empty");
        }
        if self.bind_host.trim().is_empty() {
            return EldError::validation_error("bind_host", "empty", "Bind host cannot be empty");
        }
        if self.wallet_name.trim().is_empty() {
            return EldError::validation_error(
                "wallet_name",
                "empty",
                "Wallet name cannot be empty",
            );
        }
        if self.db_path.trim().is_empty() {
            return EldError::validation_error("db_path", "empty", "Database path cannot be empty");
        }
        if self.drip_base_units == 0 {
            return EldError::validation_error(
                "drip_base_units",
                "0",
                "Drip amount must be greater than zero",
            );
        }
        Ok(())
    }

    pub fn node_url(&self) -> Result<String, EldError> {
        resolve_node_base_url(None, &self.node_host, NODE_PORT)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind_host, self.bind_port)
    }

    /// Build a [`ClientConfig`] for [`eld_client::ChainClient`]. Unused client fields
    /// get placeholders so the type is complete.
    pub fn to_client_config(&self) -> ClientConfig {
        ClientConfig {
            node_host: self.node_host.clone(),
            node_port: NODE_PORT.to_string(),
            faucet_host: self.bind_host.clone(),
            faucet_port: self.bind_port.to_string(),
            faucet_end_point: REQUEST_PATH.to_string(),
            faucet_url: None,
            app_port: APP_PORT.to_string(),
            node_url: None,
            app_url: None,
            chain_id: self.chain_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn json_defaults_fill_optional_fields() {
        let _guard = ENV_LOCK.lock().unwrap();
        for key in [
            "FAUCET_BIND_HOST",
            "FAUCET_BIND_PORT",
            "FAUCET_DB_PATH",
            "FAUCET_WALLET_PATH",
            "FAUCET_WALLET_NAME",
        ] {
            env::remove_var(key);
        }

        let config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        assert_eq!(config.bind_host, "0.0.0.0");
        assert_eq!(config.bind_port, 8080);
        assert_eq!(config.drip_base_units, 1_000_000_000);
        assert_eq!(config.address_daily_drips, 1);
        assert_eq!(config.ip_hourly_requests, 3);
        assert_eq!(config.hot_wallet_reserve, 10_000_000_000);
        assert_eq!(config.db_path, "data/faucet.db");
        assert_eq!(config.wallet_name, "wallet-faucet-1");
    }

    #[test]
    fn env_overrides_bind_and_wallet() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("FAUCET_BIND_HOST", "127.0.0.1");
        env::set_var("FAUCET_BIND_PORT", "9090");
        env::set_var("FAUCET_DB_PATH", "tmp/faucet.db");
        env::set_var("FAUCET_WALLET_PATH", "tmp/wallets.json");
        env::set_var("FAUCET_WALLET_NAME", "wallet-custom");

        let mut config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        config.wallet_path = PathBuf::from(WALLETS_PATH);
        config.apply_env_overrides().expect("env");

        assert_eq!(config.bind_host, "127.0.0.1");
        assert_eq!(config.bind_port, 9090);
        assert_eq!(config.db_path, "tmp/faucet.db");
        assert_eq!(config.wallet_path, PathBuf::from("tmp/wallets.json"));
        assert_eq!(config.wallet_name, "wallet-custom");
        assert_eq!(config.bind_addr(), "127.0.0.1:9090");

        for key in [
            "FAUCET_BIND_HOST",
            "FAUCET_BIND_PORT",
            "FAUCET_DB_PATH",
            "FAUCET_WALLET_PATH",
            "FAUCET_WALLET_NAME",
        ] {
            env::remove_var(key);
        }
    }
}
