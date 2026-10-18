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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TendermintDiskSnapshot {
    pub node_id: String,
    pub home_dir_bytes: u64,
    pub data_dir_bytes: u64,
    pub blockstore_db_bytes: u64,
    pub state_db_bytes: u64,
    pub cs_wal_bytes: u64,
    pub evidence_db_bytes: u64,
    pub tx_index_db_bytes: u64,
    pub volume_total_bytes: Option<u64>,
    pub volume_available_bytes: Option<u64>,
    pub top_files: Vec<TopFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbciDiskSnapshot {
    pub node_id: String,
    pub data_dir_bytes: u64,
    pub rocksdb_bytes: u64,
    pub capacity_bytes: u64,
    pub volume_total_bytes: Option<u64>,
    pub volume_available_bytes: Option<u64>,
    pub top_files: Vec<TopFile>,
    pub rocksdb_inventory: Option<RocksDbInventory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RocksDbInventory {
    pub cado_type_counts: Vec<CadoTypeCount>,
    pub indexed_transactions_count: Option<u64>,
    pub indexed_events_count: Option<u64>,
    pub cf_sst_bytes: Vec<CfSstBytes>,
    pub pinboard_blob_cf_sst_bytes: Option<u64>,
    pub other_cf_sst_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadoTypeCount {
    pub cado_type: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CfSstBytes {
    pub column_family: String,
    pub sst_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopFile {
    pub relative_path: String,
    pub size_bytes: u64,
}
