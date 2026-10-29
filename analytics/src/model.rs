use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionRun {
    pub collected_at: DateTime<Utc>,
    /// Chain tip at sample time (`sync_info.latest_block_height` from Tendermint status).
    /// `None` when the RPC was unreachable or returned an unusable response.
    pub block_height: Option<u64>,
    pub tendermint_nodes: Vec<TendermintDiskSnapshot>,
    pub abci_apps: Vec<AbciDiskSnapshot>,
}

/// Folder sizes for one Tendermint node. These are file sizes, not a database open.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TendermintDiskSnapshot {
    pub node_id: String,
    pub data_dir_bytes: u64,
    pub tx_index_db_bytes: u64,
    pub evidence_db_bytes: u64,
}

/// Folder sizes for one ABCI app. `rocksdb_bytes` is the size of the directory, not an open database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbciDiskSnapshot {
    pub node_id: String,
    pub data_dir_bytes: u64,
    pub rocksdb_bytes: u64,
    pub capacity_bytes: u64,
}
