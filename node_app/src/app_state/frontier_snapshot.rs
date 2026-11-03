//! In-memory frontier snapshots.
//!
//! Each value is a zstd-compressed [`eld_tendermint_proto::types::AppStateSnapshot`].
//! This store is not written to the database.

use super::AppState;
use eld_common::error::EldError;
use eld_common::validator::{ActiveCapacityValidator, CapacityValidatorInfo, ValidatorInfo};
use eld_tendermint_proto::types::{
    AppActiveCapacityValidator, AppCapacityValidator, AppNamespace, AppStateRecord, AppValidator,
};
use prost::Message;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

const ZSTD_LEVEL: i32 = 3;

/// How far behind the inserted frontier a snapshot may be before it is dropped.
const FRONTIER_SNAPSHOT_BUFFER_BLOCKS: i64 = 100;

/// Compressed frontier snapshots keyed by frontier. Memory only.
///
/// A stored frontier more than 100 blocks behind the one passed to `insert` is dropped.
#[derive(Clone)]
pub struct FrontierSnapshotStore {
    inner: Arc<Mutex<HashMap<i64, Vec<u8>>>>,
}

impl FrontierSnapshotStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Store `compressed` at `frontier`.
    ///
    /// Drops any stored frontier more than [`FRONTIER_SNAPSHOT_BUFFER_BLOCKS`] behind `frontier`.
    pub fn insert(&self, frontier: i64, compressed: Vec<u8>) {
        let mut snapshots = self.lock();
        snapshots.retain(|&stored, _| {
            stored.saturating_add(FRONTIER_SNAPSHOT_BUFFER_BLOCKS) >= frontier
        });
        snapshots.insert(frontier, compressed);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get(&self, frontier: i64) -> Option<Vec<u8>> {
        self.lock().get(&frontier).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Vec<u8>>> {
        self.inner.lock().expect("frontier snapshot store lock")
    }
}

/// Encodes `state` as [`eld_tendermint_proto::types::AppStateSnapshot`].
pub fn encode(state: &AppState) -> Result<Vec<u8>, EldError> {
    let envelope = &state.envelope;
    let cache = envelope.committed_cado_cache.without_infrastructure();
    let mut records = Vec::with_capacity(cache.len());
    for (key, body) in cache.iter() {
        records.push(AppStateRecord {
            key,
            value: body.serialize_bin()?,
        });
    }
    records.sort_by(|left, right| left.key.cmp(&right.key));

    let message = eld_tendermint_proto::types::AppStateSnapshot {
        block_height: envelope.block_height,
        app_hash: state.app_hash().bytes()?.to_vec(),
        chain_id: state.chain_id.clone(),
        current_epoch: envelope.current_epoch,
        state_trie_root: envelope.state_trie.root_hash().to_vec(),
        frontier: envelope.frontier,
        records,
        epoch_heights: envelope.epoch_records_index.iter().copied().collect(),
        namespaces: envelope
            .namespace_registry_index
            .values()
            .map(|record| AppNamespace {
                namespace_slug: record.namespace_slug.clone(),
                owner: record.owner.as_bytes().to_vec(),
                registered_height: record.registered_height,
            })
            .collect(),
        validators: envelope.validators.iter().map(validator_to_proto).collect(),
        active_validators: envelope
            .active_validators
            .iter()
            .map(validator_to_proto)
            .collect(),
        capacity_validators: envelope
            .capacity_validators
            .iter()
            .map(capacity_validator_to_proto)
            .collect(),
        active_capacity_validator: envelope
            .active_capacity_validator
            .as_ref()
            .map(active_capacity_to_proto),
    };
    Ok(message.encode_to_vec())
}

/// zstd-compresses [`encode`] bytes.
pub fn compress(raw: &[u8]) -> Result<Vec<u8>, EldError> {
    zstd::stream::encode_all(raw, ZSTD_LEVEL).map_err(|err| EldError::StorageError {
        operation: "compress_frontier_snapshot".to_string(),
        details: err.to_string(),
    })
}

/// Encodes `state`, compresses it, and inserts the bytes at its frontier.
pub fn store_frontier_snapshot(store: &FrontierSnapshotStore, state: &AppState) {
    let frontier = state.envelope.frontier;
    let raw = match encode(state) {
        Ok(bytes) => bytes,
        Err(err) => {
            error!(
                frontier,
                error = %err,
                "frontier snapshot encode failed"
            );
            return;
        }
    };
    match compress(&raw) {
        Ok(compressed) => {
            info!(
                frontier,
                bytes = compressed.len(),
                "stored frontier snapshot in memory"
            );
            store.insert(frontier, compressed);
        }
        Err(err) => {
            error!(
                frontier,
                error = %err,
                "frontier snapshot compression failed"
            );
        }
    }
}

fn validator_to_proto(validator: &ValidatorInfo) -> AppValidator {
    AppValidator {
        address: validator.address.as_bytes().to_vec(),
        stake: validator.stake.amount().to_string(),
        public_key: validator.public_key.clone(),
    }
}

fn capacity_validator_to_proto(validator: &CapacityValidatorInfo) -> AppCapacityValidator {
    AppCapacityValidator {
        address: validator.address.as_bytes().to_vec(),
        stake: validator.stake.amount().to_string(),
        public_key: validator.public_key.as_bytes().to_vec(),
        storage_capacity: validator.storage_capacity,
        merkle_root: validator.merkle_root.map(|bytes| bytes.to_vec()),
        seed: validator.seed.map(|bytes| bytes.to_vec()),
        chunk_count: validator.chunk_count,
        registered_at: validator.registered_at,
        last_merkle_root_update: validator.last_merkle_root_update,
        registered_block: validator.registered_block,
        registration_duration: validator.registration_duration,
    }
}

fn active_capacity_to_proto(validator: &ActiveCapacityValidator) -> AppActiveCapacityValidator {
    AppActiveCapacityValidator {
        validator_address: validator.validator_address.as_bytes().to_vec(),
        epoch: validator.epoch,
        challenged_providers: validator
            .challenged_providers
            .iter()
            .map(|address| address.as_bytes().to_vec())
            .collect(),
        subscribed_topics: validator.subscribed_topics.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get_round_trip() {
        let store = FrontierSnapshotStore::new();
        assert!(store.get(4).is_none());
        store.insert(4, vec![1, 2, 3]);
        assert_eq!(store.get(4), Some(vec![1, 2, 3]));
    }

    #[test]
    fn frontier_within_buffer_keeps_the_previous_snapshot() {
        let store = FrontierSnapshotStore::new();
        store.insert(4, vec![1]);
        store.insert(104, vec![2]);
        assert_eq!(store.get(4), Some(vec![1]));
        assert_eq!(store.get(104), Some(vec![2]));
    }

    #[test]
    fn frontier_more_than_buffer_behind_is_dropped() {
        let store = FrontierSnapshotStore::new();
        store.insert(4, vec![1]);
        store.insert(105, vec![2]);
        assert!(store.get(4).is_none());
        assert_eq!(store.get(105), Some(vec![2]));
    }

    #[test]
    fn older_frontier_keeps_the_newer_snapshot() {
        let store = FrontierSnapshotStore::new();
        store.insert(200, vec![1]);
        store.insert(50, vec![2]);
        assert_eq!(store.get(200), Some(vec![1]));
        assert_eq!(store.get(50), Some(vec![2]));
    }

    #[test]
    fn encode_is_the_tendermint_proto() {
        use prost::Message;

        let mut state = super::super::AppState::default();
        state.set_app_hash([0x11; 32]);
        state.chain_id = "eld-dev".to_string();
        state.envelope.init_empty_trie();
        state.envelope.block_height = 20;
        state.envelope.frontier = 17;
        let raw = super::encode(&state).expect("encode");
        let message =
            eld_tendermint_proto::types::AppStateSnapshot::decode(raw.as_slice()).expect("proto");
        assert_eq!(message.frontier, 17);
        assert_eq!(message.block_height, 20);
        assert_eq!(message.chain_id, "eld-dev");
        assert_eq!(message.app_hash, [0x11; 32]);
        let compressed = super::compress(&raw).expect("compress");
        let decoded = zstd::stream::decode_all(compressed.as_slice()).expect("decompress");
        assert_eq!(decoded, raw);
    }

    #[test]
    fn same_frontier_overwrites() {
        let store = FrontierSnapshotStore::new();
        store.insert(10, vec![1]);
        store.insert(10, vec![2]);
        assert_eq!(store.get(10), Some(vec![2]));
    }
}
