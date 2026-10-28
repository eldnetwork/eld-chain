use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionRun {
    pub collected_at: DateTime<Utc>,
    /// Chain tip at sample time (`sync_info.latest_block_height` from Tendermint status).
    /// `None` when the RPC was unreachable or returned an unusable response.
    pub block_height: Option<u64>,
    /// Linux VM memory from `/proc/meminfo` inside the sidecar.
    /// `None` when that file cannot be read.
    pub vm_memory: Option<VmMemorySnapshot>,
    pub tendermint_nodes: Vec<TendermintDiskSnapshot>,
    pub abci_apps: Vec<AbciDiskSnapshot>,
    pub tendermint_memory: Vec<TendermintMemorySnapshot>,
    pub abci_memory: Vec<AbciMemorySnapshot>,
}

/// Bytes from `/proc/meminfo`. File-cache and anonymous RAM are separate:
/// `cached_bytes` is disk data sitting in the page cache, `anon_pages_bytes` is process heap.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmMemorySnapshot {
    pub mem_total_bytes: u64,
    pub mem_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
    pub anon_pages_bytes: u64,
    pub cached_bytes: u64,
}

/// cgroup memory for one container. Fields are `None` when the Docker API did not return them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProcessMemory {
    pub usage_bytes: Option<u64>,
    pub anon_bytes: Option<u64>,
    pub file_bytes: Option<u64>,
    pub swap_bytes: Option<u64>,
    pub limit_bytes: Option<u64>,
    pub peak_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TendermintMemorySnapshot {
    pub node_id: String,
    pub memory: ProcessMemory,
    /// Unconfirmed transactions from Tendermint `num_unconfirmed_txs`.
    pub mempool_txs: Option<u64>,
    pub mempool_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbciMemorySnapshot {
    pub node_id: String,
    pub memory: ProcessMemory,
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
