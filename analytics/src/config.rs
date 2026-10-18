use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "eld-analytics",
    about = "Hourly disk and memory analytics for Eld nodes"
)]
pub struct Config {
    #[arg(
        long,
        env = "ANALYTICS_DB_PATH",
        default_value = "/data/analytics.rocksdb"
    )]
    pub db_path: PathBuf,

    #[arg(long, env = "ANALYTICS_LISTEN", default_value = "0.0.0.0:9002")]
    pub listen: String,

    /// Seconds to wait after process start before the first collection run.
    #[arg(long, env = "FIRST_COLLECTION_DELAY_SECS", default_value = "180")]
    pub first_collection_delay_secs: u64,

    #[arg(long, env = "COLLECTION_INTERVAL_SECS", default_value = "3600")]
    pub interval_secs: u64,

    #[arg(
        long,
        env = "NODE_IDS",
        default_value = "1,2,3,4",
        value_delimiter = ','
    )]
    pub node_ids: Vec<String>,

    #[arg(long, env = "TENDERMINT_PATH_PREFIX", default_value = "/tendermint-")]
    pub tendermint_path_prefix: String,

    /// Relative data dir under each Tendermint home (`data` for full TMHOME mounts).
    #[arg(long, env = "TENDERMINT_DATA_SUBDIR", default_value = "data")]
    pub tendermint_data_subdir: String,

    #[arg(long, env = "ABCI_PATH_PREFIX", default_value = "/eld-")]
    pub abci_path_prefix: String,

    /// Tendermint / node RPC base URL used to read `latest_block_height`.
    #[arg(
        long,
        env = "TENDERMINT_RPC_URL",
        default_value = "http://tendermint-1:26657"
    )]
    pub tendermint_rpc_url: String,

    /// Docker Engine socket. Per-container memory is left empty when this cannot be opened.
    #[arg(long, env = "DOCKER_SOCKET", default_value = "/var/run/docker.sock")]
    pub docker_socket: PathBuf,

    #[arg(
        long,
        env = "TENDERMINT_CONTAINER_PREFIX",
        default_value = "tendermint-"
    )]
    pub tendermint_container_prefix: String,

    #[arg(long, env = "ABCI_CONTAINER_PREFIX", default_value = "eld-app-")]
    pub abci_container_prefix: String,

    /// Hostname prefix for per-node mempool RPC: `http://{prefix}{id}:{port}`.
    #[arg(
        long,
        env = "TENDERMINT_RPC_HOST_PREFIX",
        default_value = "tendermint-"
    )]
    pub tendermint_rpc_host_prefix: String,

    #[arg(long, env = "TENDERMINT_RPC_PORT", default_value = "26657")]
    pub tendermint_rpc_port: u16,
}

impl Config {
    pub fn tendermint_container_name(&self, id: &str) -> String {
        format!("{}{id}", self.tendermint_container_prefix)
    }

    pub fn abci_container_name(&self, id: &str) -> String {
        format!("{}{id}", self.abci_container_prefix)
    }

    pub fn tendermint_rpc_url_for(&self, id: &str) -> String {
        format!(
            "http://{}{id}:{}",
            self.tendermint_rpc_host_prefix, self.tendermint_rpc_port
        )
    }
}
