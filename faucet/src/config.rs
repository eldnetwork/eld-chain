use eld_client::config::{ClientConfig, WALLETS_PATH};
use eld_client::endpoint::resolve_node_base_url;
use eld_common::error::EldError;
use eld_common::validation::{validate_ip_or_hostname, validate_port};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_FAUCET_CONFIG_PATH: &str = "config/faucet_config.json";

/// Default Tendermint RPC port when a `node_hosts` entry omits `:port`.
pub const NODE_PORT: &str = "26657";
/// Placeholder app REST port for [`ClientConfig`] (unused by the faucet binary).
pub const APP_PORT: &str = "9001";
/// GET health check path.
pub const HEALTH_PATH: &str = "/health";
/// GET readiness path (wallet, database, Tendermint quorum).
pub const READY_PATH: &str = "/ready";
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

fn default_min_synced() -> u32 {
    3
}

fn default_max_block_age_secs() -> u64 {
    15
}

fn default_poll_interval_secs() -> u64 {
    2
}

fn default_node_hosts() -> Vec<String> {
    Vec::new()
}

/// One Tendermint RPC endpoint: host plus port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEndpoint {
    pub host: String,
    pub port: String,
}

impl NodeEndpoint {
    /// Parse `host` or `host:port`. Port defaults to [`NODE_PORT`].
    pub fn parse(entry: &str) -> Result<Self, EldError> {
        let entry = entry.trim();
        if entry.is_empty() {
            EldError::validation_error(
                "node_hosts",
                "empty",
                "Tendermint RPC host entry cannot be empty",
            )?;
        }
        if let Some((host, port)) = split_host_port(entry) {
            validate_ip_or_hostname(host)?;
            validate_port(port)?;
            return Ok(Self {
                host: host.to_string(),
                port: port.to_string(),
            });
        }
        validate_ip_or_hostname(entry)?;
        Ok(Self {
            host: entry.to_string(),
            port: NODE_PORT.to_string(),
        })
    }

    pub fn rpc_url(&self) -> Result<String, EldError> {
        resolve_node_base_url(None, &self.host, &self.port)
    }
}

/// Split `host:port` when the suffix is an all-digit port. Otherwise `None`
/// (caller treats the whole string as a host).
fn split_host_port(entry: &str) -> Option<(&str, &str)> {
    let (host, port) = entry.rsplit_once(':')?;
    if host.is_empty() || port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((host, port))
}

/// Fields the faucet binary reads from disk (plus env overrides).
#[derive(Debug, Clone, Deserialize)]
pub struct FaucetConfig {
    pub node_host: String,
    /// Tendermint RPC endpoints to poll (`host` or `host:port`). If empty after
    /// load, filled from `node_host`.
    #[serde(default = "default_node_hosts")]
    pub node_hosts: Vec<String>,
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
    #[serde(default = "default_reserve")]
    pub hot_wallet_reserve: u64,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_wallet_name")]
    pub wallet_name: String,
    #[serde(default = "default_min_synced")]
    pub min_synced: u32,
    #[serde(default = "default_max_block_age_secs")]
    pub max_block_age_secs: u64,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
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
        config.normalize_node_hosts();

        config.validate().map_err(|e| EldError::ConfigError {
            file: path.to_string(),
            details: format!("Configuration validation failed for '{path}': {e}"),
        })?;

        Ok(config)
    }

    /// If `node_hosts` is empty, use `node_host` as the sole entry and clamp
    /// `min_synced` so a single-host local config still validates.
    pub fn normalize_node_hosts(&mut self) {
        if self.node_hosts.is_empty() {
            self.node_hosts = vec![self.node_host.clone()];
            if self.min_synced as usize > self.node_hosts.len() {
                self.min_synced = self.node_hosts.len() as u32;
            }
        }
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
        if let Ok(hosts) = env::var("FAUCET_NODE_HOSTS") {
            let parsed: Vec<String> = hosts
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if parsed.is_empty() {
                return Err("FAUCET_NODE_HOSTS is empty".to_string());
            }
            self.node_hosts = parsed;
        }
        Ok(())
    }

    /// Parsed `node_hosts` entries (`host` or `host:port`).
    pub fn node_endpoints(&self) -> Result<Vec<NodeEndpoint>, EldError> {
        self.node_hosts
            .iter()
            .map(|entry| NodeEndpoint::parse(entry))
            .collect()
    }

    pub fn validate(&self) -> Result<(), EldError> {
        if self.node_hosts.is_empty() {
            return EldError::validation_error(
                "node_hosts",
                "empty",
                "At least one Tendermint RPC host is required",
            );
        }
        let endpoints = self.node_endpoints()?;
        for ep in &endpoints {
            ep.rpc_url()?;
        }
        if self.min_synced < 1 {
            return EldError::validation_error(
                "min_synced",
                &self.min_synced.to_string(),
                "min_synced must be at least 1",
            );
        }
        if self.min_synced as usize > self.node_hosts.len() {
            return EldError::validation_error(
                "min_synced",
                &self.min_synced.to_string(),
                &format!(
                    "min_synced ({}) cannot exceed node_hosts length ({})",
                    self.min_synced,
                    self.node_hosts.len()
                ),
            );
        }
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

    /// Resolve each configured endpoint to an RPC base URL.
    pub fn node_urls(&self) -> Result<Vec<String>, EldError> {
        self.node_endpoints()?
            .iter()
            .map(NodeEndpoint::rpc_url)
            .collect()
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind_host, self.bind_port)
    }

    /// Build a [`ClientConfig`] for [`eld_client::ChainClient`] aimed at one RPC.
    pub fn to_client_config(&self, node_host: &str, node_port: &str) -> ClientConfig {
        ClientConfig {
            node_host: node_host.to_string(),
            node_port: node_port.to_string(),
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
    fn parse_host_defaults_port() {
        let ep = NodeEndpoint::parse("127.0.0.1").expect("parse");
        assert_eq!(ep.host, "127.0.0.1");
        assert_eq!(ep.port, "26657");
    }

    #[test]
    fn parse_host_with_port() {
        let ep = NodeEndpoint::parse("tendermint-2:26667").expect("parse");
        assert_eq!(ep.host, "tendermint-2");
        assert_eq!(ep.port, "26667");
        assert_eq!(ep.rpc_url().expect("url"), "http://tendermint-2:26667/");
    }

    #[test]
    fn json_defaults_fill_optional_fields() {
        let _guard = ENV_LOCK.lock().unwrap();
        for key in [
            "FAUCET_BIND_HOST",
            "FAUCET_BIND_PORT",
            "FAUCET_DB_PATH",
            "FAUCET_WALLET_PATH",
            "FAUCET_WALLET_NAME",
            "FAUCET_NODE_HOSTS",
        ] {
            env::remove_var(key);
        }

        let mut config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        config.normalize_node_hosts();
        assert_eq!(config.bind_host, "0.0.0.0");
        assert_eq!(config.bind_port, 8080);
        assert_eq!(config.drip_base_units, 1_000_000_000);
        assert_eq!(config.address_daily_drips, 1);
        assert_eq!(config.ip_hourly_requests, 3);
        assert_eq!(config.hot_wallet_reserve, 10_000_000_000);
        assert_eq!(config.db_path, "data/faucet.db");
        assert_eq!(config.wallet_name, "wallet-faucet-1");
        assert_eq!(config.node_hosts, vec!["127.0.0.1".to_string()]);
        // Single-host fallback clamps default min_synced (3) down to 1.
        assert_eq!(config.min_synced, 1);
        assert_eq!(config.max_block_age_secs, 15);
        assert_eq!(config.poll_interval_secs, 2);
        config.validate().expect("validate");
    }

    #[test]
    fn node_host_only_still_validates() {
        let mut config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        config.normalize_node_hosts();
        config.validate().expect("validate");
        assert_eq!(config.node_hosts.len(), 1);
    }

    #[test]
    fn four_nodes_resolve_distinct_ports() {
        let config: FaucetConfig = serde_json::from_str(
            r#"{
                "node_host": "127.0.0.1:26657",
                "node_hosts": [
                    "127.0.0.1:26657",
                    "127.0.0.1:26667",
                    "127.0.0.1:26677",
                    "127.0.0.1:26687"
                ],
                "chain_id": "eld-testnet-tempelhof"
            }"#,
        )
        .expect("parse");
        config.validate().expect("validate");
        assert_eq!(
            config.node_urls().expect("urls"),
            vec![
                "http://127.0.0.1:26657/".to_string(),
                "http://127.0.0.1:26667/".to_string(),
                "http://127.0.0.1:26677/".to_string(),
                "http://127.0.0.1:26687/".to_string(),
            ]
        );
    }

    #[test]
    fn empty_node_hosts_fails_validate() {
        let config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "node_hosts": [], "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        // Without normalize, empty list fails.
        assert!(config.validate().is_err());
    }

    #[test]
    fn min_synced_exceeds_hosts_fails_validate() {
        let mut config: FaucetConfig = serde_json::from_str(
            r#"{
                "node_host": "127.0.0.1",
                "node_hosts": ["a", "b", "c", "d"],
                "min_synced": 5,
                "chain_id": "eld-testnet-tempelhof"
            }"#,
        )
        .expect("parse");
        // Hosts "a".."d" are invalid hostnames for resolve — use real hosts.
        config.node_hosts = vec![
            "127.0.0.1".into(),
            "127.0.0.2".into(),
            "127.0.0.3".into(),
            "127.0.0.4".into(),
        ];
        config.min_synced = 5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn env_overrides_bind_and_wallet() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("FAUCET_BIND_HOST", "127.0.0.1");
        env::set_var("FAUCET_BIND_PORT", "9090");
        env::set_var("FAUCET_DB_PATH", "tmp/faucet.db");
        env::set_var("FAUCET_WALLET_PATH", "tmp/wallets.json");
        env::set_var("FAUCET_WALLET_NAME", "wallet-custom");
        env::remove_var("FAUCET_NODE_HOSTS");

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
            "FAUCET_NODE_HOSTS",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn env_overrides_node_hosts() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(
            "FAUCET_NODE_HOSTS",
            "tendermint-1:26657, tendermint-2:26667 ,tendermint-3:26677",
        );
        for key in [
            "FAUCET_BIND_HOST",
            "FAUCET_BIND_PORT",
            "FAUCET_DB_PATH",
            "FAUCET_WALLET_PATH",
            "FAUCET_WALLET_NAME",
        ] {
            env::remove_var(key);
        }

        let mut config: FaucetConfig = serde_json::from_str(
            r#"{ "node_host": "127.0.0.1", "chain_id": "eld-testnet-tempelhof" }"#,
        )
        .expect("parse");
        config.apply_env_overrides().expect("env");
        assert_eq!(
            config.node_hosts,
            vec![
                "tendermint-1:26657".to_string(),
                "tendermint-2:26667".to_string(),
                "tendermint-3:26677".to_string()
            ]
        );
        env::remove_var("FAUCET_NODE_HOSTS");
    }
}
