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
