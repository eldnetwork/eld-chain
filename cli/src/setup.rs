//! Load and save `eld-cli-config.json` without requiring a faucet address.

use eld_client::api::abci::AbciHttpApi;
use eld_client::config::ClientConfig;
use eld_client::endpoint::{
    normalize_http_base_url, resolve_faucet_request_url, resolve_node_base_url,
};
use eld_common::error::{EldError, ErrorBuilder};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;

const NODE_PROMPT: &str = "Node address (IP or URL): ";
const FAUCET_PROMPT: &str = "Faucet address (IP or URL): ";
const NODE_HINT: &str = "eld-cli config node <ip-or-url>";
const FAUCET_HINT: &str = "eld-cli config faucet <ip-or-url>";
const CHAIN_ID_HINT: &str = "Run `eld-cli config node <ip-or-url>` again when the node is up.";
const DEFAULT_NODE_PORT: &str = "26657";
const DEFAULT_APP_PORT: &str = "9001";
const DEFAULT_FAUCET_PORT: &str = "8080";
const DEFAULT_FAUCET_ENDPOINT: &str = "/faucet/request";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredCliConfig {
    #[serde(default)]
    node_host: String,
    #[serde(default = "default_node_port")]
    node_port: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node_url: Option<String>,
    #[serde(
        default = "default_app_port",
        rename = "app_port",
        alias = "upload_port"
    )]
    app_port: String,
    #[serde(
        default,
        rename = "app_url",
        alias = "upload_url",
        skip_serializing_if = "Option::is_none"
    )]
    app_url: Option<String>,
    #[serde(default)]
    faucet_host: String,
    #[serde(default)]
    faucet_port: String,
    #[serde(default = "default_faucet_endpoint")]
    faucet_end_point: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    faucet_url: Option<String>,
    #[serde(default)]
    chain_id: String,
}

fn default_node_port() -> String {
    DEFAULT_NODE_PORT.to_string()
}

fn default_app_port() -> String {
    DEFAULT_APP_PORT.to_string()
}

fn default_faucet_endpoint() -> String {
    DEFAULT_FAUCET_ENDPOINT.to_string()
}

impl StoredCliConfig {
    fn empty() -> Self {
        Self {
            node_host: String::new(),
            node_port: DEFAULT_NODE_PORT.to_string(),
            node_url: None,
            app_port: DEFAULT_APP_PORT.to_string(),
            app_url: None,
            faucet_host: String::new(),
            faucet_port: String::new(),
            faucet_end_point: DEFAULT_FAUCET_ENDPOINT.to_string(),
            faucet_url: None,
            chain_id: String::new(),
        }
    }

    fn to_client_config(&self) -> ClientConfig {
        ClientConfig {
            node_host: self.node_host.clone(),
            node_port: self.node_port.clone(),
            faucet_host: self.faucet_host.clone(),
            faucet_port: self.faucet_port.clone(),
            faucet_end_point: self.faucet_end_point.clone(),
            faucet_url: self.faucet_url.clone(),
            app_port: self.app_port.clone(),
            node_url: self.node_url.clone(),
            app_url: self.app_url.clone(),
            chain_id: self.chain_id.clone(),
        }
    }
}

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn option_blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(blank)
}

fn node_is_unset(file: &StoredCliConfig) -> bool {
    blank(&file.node_host) && option_blank(&file.node_url)
}

fn faucet_is_unset(file: &StoredCliConfig) -> bool {
    blank(&file.faucet_host) && option_blank(&file.faucet_url)
}

fn load_or_empty(path: &Path) -> Result<StoredCliConfig, EldError> {
    if !path.exists() {
        return Ok(StoredCliConfig::empty());
    }
    let text = std::fs::read_to_string(path).map_err(|err| {
        ErrorBuilder::file_system_error("read", &path.display().to_string(), &err.to_string())
    })?;
    serde_json::from_str(&text).map_err(|err| {
        ErrorBuilder::config_error(
            &path.display().to_string(),
            &format!("Failed to parse eld-cli-config.json: {err}"),
        )
    })
}

fn save(path: &Path, file: &StoredCliConfig) -> Result<(), EldError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| {
                ErrorBuilder::file_system_error(
                    "create directory",
                    &parent.display().to_string(),
                    &err.to_string(),
                )
            })?;
        }
    }
    let mut body = serde_json::to_vec_pretty(file).map_err(|err| {
        ErrorBuilder::config_error(
            &path.display().to_string(),
            &format!("Failed to write eld-cli-config.json: {err}"),
        )
    })?;
    body.push(b'\n');
    std::fs::write(path, body).map_err(|err| {
        ErrorBuilder::file_system_error("write", &path.display().to_string(), &err.to_string())
    })
}

fn is_http_url(raw: &str) -> bool {
    raw.starts_with("http://") || raw.starts_with("https://")
}

/// `host` or `host:port`. A non-numeric suffix is left as part of the host so validation rejects it.
fn split_host_port(raw: &str, default_port: &str) -> (String, String) {
    if let Some((host, port)) = raw.rsplit_once(':') {
        if !host.is_empty()
            && !host.contains(':')
            && !port.is_empty()
            && port.chars().all(|c| c.is_ascii_digit())
        {
            return (host.to_string(), port.to_string());
        }
    }
    (raw.to_string(), default_port.to_string())
}

fn set_node_address(file: &mut StoredCliConfig, raw: &str) -> Result<(), EldError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(EldError::make_validation_error(
            "node_address",
            "empty",
            format!("Node address is empty. Run: {NODE_HINT}"),
        ));
    }
    if is_http_url(raw) {
        let node_url = normalize_http_base_url(raw)?;
        file.node_url = Some(node_url);
        file.node_host.clear();
        file.node_port.clear();
    } else {
        let (host, port) = split_host_port(raw, DEFAULT_NODE_PORT);
        resolve_node_base_url(None, &host, &port)?;
        file.node_url = None;
        file.node_host = host;
        file.node_port = port;
    }
    file.app_url = None;
    file.app_port = DEFAULT_APP_PORT.to_string();
    Ok(())
}

fn set_faucet_address(file: &mut StoredCliConfig, raw: &str) -> Result<(), EldError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(EldError::make_validation_error(
            "faucet_address",
            "empty",
            format!("Faucet address is empty. Run: {FAUCET_HINT}"),
        ));
    }
    if is_http_url(raw) {
        resolve_faucet_request_url(Some(raw), "", "", DEFAULT_FAUCET_ENDPOINT)?;
        file.faucet_url = Some(raw.to_string());
        file.faucet_host.clear();
        file.faucet_port.clear();
        if blank(&file.faucet_end_point) {
            file.faucet_end_point = DEFAULT_FAUCET_ENDPOINT.to_string();
        }
    } else {
        let (host, port) = split_host_port(raw, DEFAULT_FAUCET_PORT);
        resolve_faucet_request_url(None, &host, &port, DEFAULT_FAUCET_ENDPOINT)?;
        file.faucet_url = None;
        file.faucet_host = host;
        file.faucet_port = port;
        file.faucet_end_point = DEFAULT_FAUCET_ENDPOINT.to_string();
    }
    Ok(())
}

fn read_address(interactive: bool, prompt: &str, hint: &str) -> Result<String, EldError> {
    if !interactive {
        return Err(EldError::make_validation_error(
            "address",
            "missing",
            format!("Run: {hint}"),
        ));
    }
    eprint!("{prompt}");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|err| ErrorBuilder::file_system_error("read", "stdin", &err.to_string()))?;
    let line = line.trim();
    if line.is_empty() {
        return Err(EldError::make_validation_error(
            "address",
            "empty",
            format!("Run: {hint}"),
        ));
    }
    Ok(line.to_string())
}

async fn refresh_chain_id(file: &mut StoredCliConfig) {
    let url = match file.to_client_config().get_node_url() {
        Ok(url) => url,
        Err(err) => {
            file.chain_id.clear();
            eprintln!("Node did not return a chain id ({err}). Saved the address. {CHAIN_ID_HINT}");
            return;
        }
    };
    let chain_id = match AbciHttpApi::new(url) {
        Ok(api) => api.chain_id_from_status().await,
        Err(err) => Err(err),
    };
    match chain_id {
        Ok(chain_id) if !blank(&chain_id) => file.chain_id = chain_id,
        Ok(_) => {
            file.chain_id.clear();
            eprintln!("Node returned an empty chain id. Saved the address. {CHAIN_ID_HINT}");
        }
        Err(err) => {
            file.chain_id.clear();
            eprintln!("Node did not return a chain id ({err}). Saved the address. {CHAIN_ID_HINT}");
        }
    }
}

pub(crate) async fn ensure_node(path: &Path, interactive: bool) -> Result<(), EldError> {
    let mut file = load_or_empty(path)?;
    if !node_is_unset(&file) {
        return Ok(());
    }
    let address = read_address(interactive, NODE_PROMPT, NODE_HINT)?;
    set_node_address(&mut file, &address)?;
    refresh_chain_id(&mut file).await;
    save(path, &file)
}

pub(crate) async fn ensure_faucet(path: &Path, interactive: bool) -> Result<(), EldError> {
    let mut file = load_or_empty(path)?;
    if !faucet_is_unset(&file) {
        return Ok(());
    }
    let address = read_address(interactive, FAUCET_PROMPT, FAUCET_HINT)?;
    set_faucet_address(&mut file, &address)?;
    save(path, &file)
}

pub(crate) fn load_client_config(path: &Path) -> Result<ClientConfig, EldError> {
    let file = load_or_empty(path)?;
    if node_is_unset(&file) {
        return Err(EldError::make_validation_error(
            "node_address",
            "missing",
            format!("No node address in eld-cli-config.json. Run: {NODE_HINT}"),
        ));
    }
    let config = file.to_client_config();
    config.get_node_url()?;
    config.get_app_base_url()?;
    Ok(config)
}

pub(crate) fn chain_id_missing_error() -> EldError {
    EldError::make_validation_error(
        "chain_id",
        "empty",
        format!("Chain ID is empty in eld-cli-config.json. {CHAIN_ID_HINT}"),
    )
}

pub(crate) async fn configure_node(path: &Path, address: &str) -> Result<(), EldError> {
    let mut file = load_or_empty(path)?;
    set_node_address(&mut file, address)?;
    refresh_chain_id(&mut file).await;
    save(path, &file)?;
    println!("Wrote {}", path.display());
    Ok(())
}

pub(crate) async fn configure_faucet(path: &Path, address: &str) -> Result<(), EldError> {
    let mut file = load_or_empty(path)?;
    set_faucet_address(&mut file, address)?;
    save(path, &file)?;
    println!("Wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ip_and_hostname_use_default_port_and_leave_faucet_empty() {
        let mut file = StoredCliConfig::empty();
        set_node_address(&mut file, "127.0.0.1").unwrap();
        assert_eq!(file.node_host, "127.0.0.1");
        assert_eq!(file.node_port, "26657");
        assert!(file.node_url.is_none());
        assert_eq!(file.app_port, "9001");
        assert!(file.app_url.is_none());
        assert!(faucet_is_unset(&file));
        assert_eq!(file.faucet_end_point, "/faucet/request");

        set_node_address(&mut file, "localhost").unwrap();
        assert_eq!(file.node_host, "localhost");
        assert_eq!(file.node_port, "26657");
        assert!(faucet_is_unset(&file));
    }

    #[test]
    fn node_url_clears_host_and_leaves_faucet_empty() {
        let mut file = StoredCliConfig::empty();
        file.faucet_host = "10.0.0.8".to_string();
        file.faucet_port = "8080".to_string();
        set_node_address(&mut file, "https://rpc.example.com").unwrap();
        assert!(file.node_host.is_empty());
        assert_eq!(file.node_url.as_deref(), Some("https://rpc.example.com/"));
        assert_eq!(file.app_port, "9001");
        assert_eq!(file.faucet_host, "10.0.0.8");
        assert_eq!(file.faucet_port, "8080");
        assert!(file.faucet_url.is_none());
    }

    #[test]
    fn faucet_ip_and_url_do_not_change_node_fields() {
        let mut file = StoredCliConfig::empty();
        set_node_address(&mut file, "127.0.0.1").unwrap();
        set_faucet_address(&mut file, "10.1.2.3").unwrap();
        assert_eq!(file.node_host, "127.0.0.1");
        assert_eq!(file.node_port, "26657");
        assert!(file.node_url.is_none());
        assert_eq!(file.faucet_host, "10.1.2.3");
        assert_eq!(file.faucet_port, "8080");
        assert_eq!(file.faucet_end_point, "/faucet/request");
        assert!(file.faucet_url.is_none());

        set_faucet_address(&mut file, "https://faucet.example.com").unwrap();
        assert_eq!(file.node_host, "127.0.0.1");
        assert_eq!(file.node_port, "26657");
        assert!(file.node_url.is_none());
        assert!(file.faucet_host.is_empty());
        assert_eq!(
            file.faucet_url.as_deref(),
            Some("https://faucet.example.com")
        );
    }

    #[test]
    fn saved_node_file_keeps_faucet_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("eld-cli-config.json");
        let mut file = StoredCliConfig::empty();
        set_node_address(&mut file, "127.0.0.1").unwrap();
        save(&path, &file).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"node_host\": \"127.0.0.1\""), "{text}");
        assert!(text.contains("\"faucet_host\": \"\""), "{text}");
        assert!(text.contains("\"faucet_port\": \"\""), "{text}");
        assert!(!text.contains("faucet_url"), "{text}");
        let loaded = load_or_empty(&path).unwrap();
        assert!(!node_is_unset(&loaded));
        assert!(faucet_is_unset(&loaded));
        assert_eq!(loaded.faucet_end_point, "/faucet/request");
    }

    #[test]
    fn node_host_port_is_accepted() {
        let mut file = StoredCliConfig::empty();
        set_node_address(&mut file, "127.0.0.1:26658").unwrap();
        assert_eq!(file.node_host, "127.0.0.1");
        assert_eq!(file.node_port, "26658");
    }
}
