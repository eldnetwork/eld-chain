use crate::abci_interface::snapshot::codec::{serialize_abci_snapshot, AbciSnapshot};
use crate::app_state::app_state_snapshot::AppStateSnapshot;
use crate::app_state::AppStateTip;
use crate::storage::traits::{
    CADOStorage, SnapshotChunk, SnapshotMetadata, SnapshotStateCounts, SnapshotStorage,
};
use eld_common::error::EldError;
use eld_common::{
    cado::{CadoPath, CadoPathKey, CadoType},
    constants::cado::LATEST,
};
use lz4::block::compress;
#[cfg(test)]
use lz4::block::decompress;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Number of complete ABCI snapshots to retain after each successful create.
const SNAPSHOT_KEEP_LAST_N: u32 = 2;

/// Outcome of [`SnapshotManager::create_snapshot_and_prune`].
#[derive(Debug)]
pub enum CreateSnapshotAndPruneOutcome {
    /// A snapshot build was already running; this height was skipped.
    SkippedAlreadyInFlight,
    /// Snapshot created, verified, and prune completed.
    Created,
    /// Snapshot created and verified, but prune failed.
    CreatedButPruneFailed(EldError),
}

#[derive(Debug)]
pub struct SnapshotManager<S>
where
    S: SnapshotStorage + Send + Sync + 'static,
{
    storage: Arc<S>,
    build_in_flight: AtomicBool,
}

impl<S> SnapshotManager<S>
where
    S: SnapshotStorage + Send + Sync + 'static,
{
    pub fn new(storage: Arc<S>) -> Self {
        Self {
            storage,
            build_in_flight: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    pub fn set_build_in_flight_for_test(&self, in_flight: bool) {
        self.build_in_flight.store(in_flight, Ordering::SeqCst);
    }

    /// Get metadata for a snapshot at the specified height
    pub async fn get_metadata(&self, height: i64) -> Result<Option<SnapshotMetadata>, EldError> {
        self.storage.get_snapshot_metadata(height)
    }

    /// Get a specific chunk from a snapshot
    pub async fn get_chunk(
        &self,
        height: i64,
        index: u32,
    ) -> Result<Option<SnapshotChunk>, EldError> {
        self.storage.get_snapshot_chunk(height, index)
    }

    /// List all available snapshots, up to the specified limit
    pub async fn list_snapshots(&self, limit: u32) -> Result<Vec<SnapshotMetadata>, EldError> {
        self.storage.list_snapshots(limit)
    }

    pub async fn create_snapshot_with_app_hash(
        &self,
        height: i64,
        data: Vec<u8>,
        app_hash: [u8; 32],
        counts: SnapshotStateCounts,
    ) -> Result<(), EldError> {
        // Split data into chunks (10MB each)
        const CHUNK_SIZE: usize = 10_000_000;
        let chunks: Vec<Vec<u8>> = data.chunks(CHUNK_SIZE).map(|c| c.to_vec()).collect();

        // Compress chunks and collect metadata
        let mut compressed_chunks = Vec::new();
        let mut chunk_hashes = Vec::new();
        let mut total_compressed_size = 0;

        for chunk in chunks {
            // Compress chunk using LZ4
            let compressed = compress(&chunk, None, true).map_err(|e| EldError::StorageError {
                operation: "compress_chunk".to_string(),
                details: format!("Failed to compress chunk: {e}"),
            })?;
            total_compressed_size += compressed.len();

            // Calculate hash of original chunk
            let mut hasher = Sha256::new();
            hasher.update(&chunk);
            let hash = hex::encode(hasher.finalize());

            compressed_chunks.push(compressed);
            chunk_hashes.push(hash);
        }

        // Create metadata
        let metadata = SnapshotMetadata {
            height,
            epoch: (height / 100) as u64,
            format_version: 1,
            chunk_count: compressed_chunks.len() as u32,
            total_size: total_compressed_size as u64,
            compression: "lz4".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Failed to get current time")
                .as_secs(),
            app_hash,
            accounts_count: counts.accounts_count,
            staking_accounts_count: counts.staking_accounts_count,
            storage_staking_accounts_count: counts.storage_staking_accounts_count,
            namespaces_count: counts.namespaces_count,
            chunk_hashes,
        };

        // Store metadata
        self.storage.put_snapshot_metadata(&metadata)?;

        // Store compressed chunks
        for (i, chunk_data) in compressed_chunks.into_iter().enumerate() {
            let chunk = SnapshotChunk {
                index: i as u32,
                data: chunk_data,
                hash: metadata.chunk_hashes[i].clone(),
                size: CHUNK_SIZE as u64,
            };

            self.storage.put_snapshot_chunk(height, &chunk)?;
        }

        Ok(())
    }

    /// Store a single snapshot chunk
    ///
    /// This method stores a chunk directly, bypassing the normal snapshot creation process.
    /// It's primarily used when receiving snapshot chunks from other nodes.
    pub async fn put_chunk(&self, height: i64, chunk: &SnapshotChunk) -> Result<(), EldError> {
        // Store the chunk directly
        self.storage.put_snapshot_chunk(height, chunk)
    }

    pub async fn prune_snapshots(&self, keep_last_n: u32) -> Result<(), EldError> {
        let snapshots = self.list_snapshots(u32::MAX).await?;
        if snapshots.len() <= keep_last_n as usize {
            return Ok(());
        }

        let mut heights: Vec<i64> = snapshots.iter().map(|s| s.height).collect();
        heights.sort_unstable_by(|a, b| b.cmp(a));

        for height in heights.iter().skip(keep_last_n as usize) {
            self.delete_snapshot(*height).await?;
        }

        Ok(())
    }

    pub async fn delete_snapshot(&self, height: i64) -> Result<(), EldError> {
        if let Some(metadata) = self.get_metadata(height).await? {
            for i in 0..metadata.chunk_count {
                self.storage.delete_snapshot_chunk(height, i)?;
            }
            self.storage.delete_snapshot_metadata(height)?;
        }
        Ok(())
    }

    /// Verify metadata exists and every chunk index `0..chunk_count` is present.
    async fn verify_snapshot_complete(&self, height: i64) -> Result<(), EldError> {
        let metadata = self
            .get_metadata(height)
            .await?
            .ok_or_else(|| EldError::StorageError {
                operation: "verify_snapshot_complete".to_string(),
                details: format!("Snapshot metadata missing for height {height}"),
            })?;

        for i in 0..metadata.chunk_count {
            if self.get_chunk(height, i).await?.is_none() {
                return Err(EldError::StorageError {
                    operation: "verify_snapshot_complete".to_string(),
                    details: format!("Missing chunk {i} for snapshot height {height}"),
                });
            }
        }

        Ok(())
    }
}

impl<S> SnapshotManager<S>
where
    S: SnapshotStorage + CADOStorage + Send + Sync + 'static,
{
    /// Create an ABCI snapshot, verify it, then prune to [`SNAPSHOT_KEEP_LAST_N`].
    ///
    /// Skips if another build is already in flight. Does not prune if create or verify fails.
    pub async fn create_snapshot_and_prune(
        &self,
        height: i64,
    ) -> Result<CreateSnapshotAndPruneOutcome, EldError> {
        if self
            .build_in_flight
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Ok(CreateSnapshotAndPruneOutcome::SkippedAlreadyInFlight);
        }

        let outcome = self.create_snapshot_and_prune_locked(height).await;
        self.build_in_flight.store(false, Ordering::Release);
        outcome
    }

    /// Create, verify, and prune while `build_in_flight` is held by the caller.
    async fn create_snapshot_and_prune_locked(
        &self,
        height: i64,
    ) -> Result<CreateSnapshotAndPruneOutcome, EldError> {
        self.create_snapshot_from_latest_state(height).await?;
        self.verify_snapshot_complete(height).await?;

        match self.prune_snapshots(SNAPSHOT_KEEP_LAST_N).await {
            Ok(()) => Ok(CreateSnapshotAndPruneOutcome::Created),
            Err(e) => Ok(CreateSnapshotAndPruneOutcome::CreatedButPruneFailed(e)),
        }
    }

    /// Build and persist an ABCI snapshot from already committed state in RocksDB.
    pub async fn create_snapshot_from_latest_state(&self, height: i64) -> Result<(), EldError> {
        let latest_app_state_tip_path =
            CadoPath::new(CadoType::AppStateTip, CadoPathKey::Name(LATEST))?;
        let app_state_tip: AppStateTip = self
            .storage
            .get_deserialized_cado_by_path(latest_app_state_tip_path)?;

        if app_state_tip.block_height != height {
            return Err(EldError::ValidationError {
                field: "app_state_tip.block_height".to_string(),
                value: app_state_tip.block_height.to_string(),
                details: format!(
                    "Latest AppStateTip height mismatch. Expected {}, got {}",
                    height, app_state_tip.block_height
                ),
            });
        }

        let app_state_snapshot_path = AppStateSnapshot::latest_path()?;
        let app_state_snapshot: AppStateSnapshot = self
            .storage
            .get_deserialized_cado_by_path(app_state_snapshot_path)?;

        if app_state_snapshot.block_height != height {
            return Err(EldError::ValidationError {
                field: "app_state_snapshot.block_height".to_string(),
                value: app_state_snapshot.block_height.to_string(),
                details: format!(
                    "Latest AppStateSnapshot height mismatch. Expected {}, got {}",
                    height, app_state_snapshot.block_height
                ),
            });
        }

        if app_state_snapshot.app_hash != app_state_tip.app_hash {
            return Err(EldError::ValidationError {
                field: "app_state_snapshot.app_hash".to_string(),
                value: hex::encode(app_state_snapshot.app_hash),
                details: "AppStateSnapshot app_hash does not match latest AppStateTip".to_string(),
            });
        }

        if app_state_snapshot.state_trie_root != app_state_tip.cado_root_hash {
            return Err(EldError::ValidationError {
                field: "app_state_snapshot.state_trie_root".to_string(),
                value: hex::encode(app_state_snapshot.state_trie_root),
                details: "AppStateSnapshot trie root does not match latest AppStateTip".to_string(),
            });
        }

        let snapshot = AbciSnapshot::new(app_state_snapshot.clone());
        let payload = serialize_abci_snapshot(&snapshot)?;
        self.create_snapshot_with_app_hash(
            height,
            payload,
            app_state_snapshot.app_hash,
            app_state_snapshot.state_counts(),
        )
        .await
    }
}

#[cfg(test)]
impl<S> SnapshotManager<S>
where
    S: SnapshotStorage + crate::storage::traits::SnapshotStorageTestExt + Send + Sync + 'static,
{
    pub async fn verify(&self, height: i64) -> Result<bool, EldError> {
        self.storage.verify_snapshot(height)
    }

    pub async fn get_snapshot(&self, height: i64) -> Result<Option<Vec<u8>>, EldError> {
        let metadata = match self.get_metadata(height).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        if !self.verify(height).await? {
            return Err(EldError::StorageError {
                operation: "verify_snapshot".to_string(),
                details: "Snapshot verification failed".to_string(),
            });
        }

        let mut data = Vec::new();
        for i in 0..metadata.chunk_count {
            let chunk = self
                .get_chunk(height, i)
                .await?
                .ok_or_else(|| EldError::StorageError {
                    operation: "get_snapshot_chunk".to_string(),
                    details: format!("Missing chunk {i}"),
                })?;

            let decompressed =
                decompress(&chunk.data, None).map_err(|e| EldError::StorageError {
                    operation: "decompress_chunk".to_string(),
                    details: format!("Failed to decompress chunk {i}: {e}"),
                })?;
            data.extend_from_slice(&decompressed);
        }

        Ok(Some(data))
    }

    pub async fn get_latest_snapshot_height(&self) -> Result<Option<i64>, EldError> {
        Ok(self
            .storage
            .get_latest_snapshot_metadata()?
            .map(|m| m.height))
    }

    pub async fn should_create_snapshot(&self, height: i64) -> bool {
        height % 100 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::rocksdb::RocksDBStorage;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_snapshot_manager_basic_operations() {
        tracing::info!("Running basic operations test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Test creating a snapshot
        let test_data = vec![1, 2, 3, 4, 5];
        manager
            .create_snapshot_with_app_hash(
                100,
                test_data.clone(),
                [0x11; 32],
                SnapshotStateCounts::default(),
            )
            .await
            .expect("Failed to create snapshot");

        // Test retrieving metadata
        let metadata = manager
            .get_metadata(100)
            .await
            .expect("Failed to get metadata")
            .expect("No metadata found for height 100");
        assert_eq!(metadata.height, 100);

        // Test retrieving the snapshot
        let retrieved = manager
            .get_snapshot(100)
            .await
            .expect("Failed to get snapshot");
        assert_eq!(retrieved, Some(test_data));

        // Test getting latest height
        assert_eq!(
            manager
                .get_latest_snapshot_height()
                .await
                .expect("Failed to get latest height"),
            Some(100)
        );
    }

    #[tokio::test]
    async fn test_snapshot_manager_multiple_snapshots() {
        tracing::info!("Running multiple snapshots test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Create multiple snapshots
        let heights = vec![100, 200, 300];
        for height in &heights {
            let data = vec![*height as u8];
            manager
                .create_snapshot_with_app_hash(
                    *height,
                    data,
                    [*height as u8; 32],
                    SnapshotStateCounts::default(),
                )
                .await
                .expect("Failed to create snapshot");
        }

        // List all snapshots
        let snapshots = manager
            .list_snapshots(10)
            .await
            .expect("Failed to list snapshots");
        assert_eq!(snapshots.len(), 3);

        // Verify all snapshots exist and can be retrieved
        for height in &heights {
            let data = manager
                .get_snapshot(*height)
                .await
                .expect("Failed to get snapshot");
            assert_eq!(data, Some(vec![*height as u8]));
        }

        // Test pruning
        manager
            .prune_snapshots(1)
            .await
            .expect("Failed to prune snapshots");

        // Verify only latest snapshot remains
        assert!(manager
            .get_snapshot(100)
            .await
            .expect("Failed to get snapshot")
            .is_none());
        assert!(manager
            .get_snapshot(200)
            .await
            .expect("Failed to get snapshot")
            .is_none());
        assert!(manager
            .get_snapshot(300)
            .await
            .expect("Failed to get snapshot")
            .is_some());

        // List snapshots after pruning
        let snapshots = manager
            .list_snapshots(10)
            .await
            .expect("Failed to list snapshots");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].height, 300);
    }

    #[tokio::test]
    async fn test_snapshot_manager_large_data() {
        tracing::info!("Running large data test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Create a large snapshot (larger than chunk size)
        let large_data: Vec<u8> = vec![1; 20_000_000]; // 20MB
        manager
            .create_snapshot_with_app_hash(
                100,
                large_data.clone(),
                [0x11; 32],
                SnapshotStateCounts::default(),
            )
            .await
            .expect("Failed to create snapshot");

        // Verify it can be retrieved correctly
        let retrieved = manager
            .get_snapshot(100)
            .await
            .expect("Failed to get snapshot");
        assert_eq!(retrieved, Some(large_data));
    }

    #[tokio::test]
    async fn test_snapshot_manager_nonexistent_snapshot() {
        tracing::info!("Running nonexistent snapshot test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Test retrieving non-existent snapshot
        assert!(manager
            .get_snapshot(999)
            .await
            .expect("Failed to get snapshot")
            .is_none());
        assert_eq!(
            manager
                .get_latest_snapshot_height()
                .await
                .expect("Failed to get latest height"),
            None
        );
    }

    #[tokio::test]
    async fn test_snapshot_manager_compression() {
        tracing::info!("Running compression test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Create a snapshot with compressible data
        let data = vec![0; 1_000_000]; // 1MB of zeros (highly compressible)
        manager
            .create_snapshot_with_app_hash(
                100,
                data.clone(),
                [0x11; 32],
                SnapshotStateCounts::default(),
            )
            .await
            .expect("Failed to create snapshot");

        // Verify compression worked by checking chunk size
        let _ = manager
            .get_metadata(100)
            .await
            .expect("Failed to get metadata");
        let chunk = manager
            .get_chunk(100, 0)
            .await
            .expect("Failed to get chunk");

        // Compressed size should be much smaller than original
        assert!(chunk.expect("Chunk not found").data.len() < data.len());
    }

    #[tokio::test]
    async fn test_snapshot_manager_epoch_boundaries() {
        tracing::info!("Running epoch boundaries test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Test at epoch boundary
        assert!(manager.should_create_snapshot(100).await);
        assert!(manager.should_create_snapshot(200).await);
        assert!(manager.should_create_snapshot(300).await);

        // Test not at epoch boundary
        assert!(!manager.should_create_snapshot(101).await);
        assert!(!manager.should_create_snapshot(199).await);
        assert!(!manager.should_create_snapshot(301).await);
    }

    #[tokio::test]
    async fn test_snapshot_manager_delete() {
        tracing::info!("Running delete test");
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        // Create a snapshot
        let data = vec![1, 2, 3, 4, 5];
        manager
            .create_snapshot_with_app_hash(100, data, [0x11; 32], SnapshotStateCounts::default())
            .await
            .expect("Failed to create snapshot");

        // Verify it exists
        assert!(manager
            .get_metadata(100)
            .await
            .expect("Failed to get metadata")
            .is_some());

        // Delete it
        manager
            .delete_snapshot(100)
            .await
            .expect("Failed to delete snapshot");

        // Verify it's gone
        assert!(manager
            .get_metadata(100)
            .await
            .expect("Failed to get metadata")
            .is_none());
        assert!(manager
            .get_chunk(100, 0)
            .await
            .expect("Failed to get chunk")
            .is_none());
    }

    /// Seed AppStateTip + AppStateSnapshot at LATEST so create_snapshot_from_latest_state succeeds.
    async fn seed_latest_state_for_height<S>(storage: &S, height: i64, app_hash: [u8; 32])
    where
        S: CADOStorage,
    {
        use crate::app_state::committed_cado_cache::CommittedCadoCache;
        use crate::app_state::state_trie::StateTrie;
        use eld_common::cado::{CADOMetadata, CadoBody};
        use std::collections::{BTreeMap, BTreeSet};

        let state_trie_root = StateTrie::empty_root_hash();
        let tip = AppStateTip {
            block_height: height,
            cado_root_hash: state_trie_root,
            app_hash,
        };
        let tip_path =
            CadoPath::new(CadoType::AppStateTip, CadoPathKey::Name(LATEST)).expect("tip path");
        let tip_cado = CadoBody::immutable(
            bincode::serialize(&tip).expect("serialize tip"),
            CADOMetadata::new(CadoType::AppStateTip, "system"),
        );
        storage
            .put_cado_type(tip_path, tip_cado)
            .expect("persist tip");

        let snapshot = AppStateSnapshot {
            block_height: height,
            app_hash,
            chain_id: "test-chain".to_string(),
            current_epoch: height / 100,
            committed_cado_cache: CommittedCadoCache::new(),
            state_trie_root,
            epoch_records_index: BTreeSet::new(),
            namespace_registry_index: BTreeMap::new(),
            validators: Vec::new(),
            active_validators: Vec::new(),
            capacity_validators: Vec::new(),
            active_capacity_validator: None,
        };
        storage
            .put_cado_type(
                AppStateSnapshot::latest_path().expect("snapshot path"),
                snapshot.to_cado().expect("snapshot to cado"),
            )
            .expect("persist app state snapshot");
    }

    #[tokio::test]
    async fn test_create_snapshot_and_prune_keeps_last_two() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage.clone());

        for height in [100_i64, 200, 300] {
            let app_hash = [height as u8; 32];
            seed_latest_state_for_height(storage.as_ref(), height, app_hash).await;
            let outcome = manager
                .create_snapshot_and_prune(height)
                .await
                .expect("create_snapshot_and_prune");
            assert!(matches!(outcome, CreateSnapshotAndPruneOutcome::Created));
        }

        let snapshots = manager.list_snapshots(10).await.expect("list_snapshots");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].height, 300);
        assert_eq!(snapshots[1].height, 200);

        assert!(manager
            .get_metadata(100)
            .await
            .expect("get metadata 100")
            .is_none());
        assert!(manager
            .get_chunk(100, 0)
            .await
            .expect("get chunk 100")
            .is_none());
        assert!(manager
            .get_metadata(200)
            .await
            .expect("get metadata 200")
            .is_some());
        assert!(manager
            .get_metadata(300)
            .await
            .expect("get metadata 300")
            .is_some());
    }

    #[tokio::test]
    async fn test_create_snapshot_and_prune_failed_create_does_not_prune() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        for height in [100_i64, 200] {
            manager
                .create_snapshot_with_app_hash(
                    height,
                    vec![height as u8],
                    [height as u8; 32],
                    SnapshotStateCounts::default(),
                )
                .await
                .expect("seed snapshot");
        }

        let result = manager.create_snapshot_and_prune(999).await;
        assert!(
            result.is_err(),
            "expected create to fail without tip/snapshot"
        );

        assert!(manager
            .get_metadata(100)
            .await
            .expect("get metadata 100")
            .is_some());
        assert!(manager
            .get_metadata(200)
            .await
            .expect("get metadata 200")
            .is_some());
        let snapshots = manager.list_snapshots(10).await.expect("list_snapshots");
        assert_eq!(snapshots.len(), 2);
    }

    #[tokio::test]
    async fn test_create_snapshot_and_prune_skips_when_in_flight() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(
            RocksDBStorage::new(temp_dir.path()).expect("Failed to create RocksDBStorage"),
        );
        let manager = SnapshotManager::new(storage);

        manager.set_build_in_flight_for_test(true);
        let outcome = manager
            .create_snapshot_and_prune(100)
            .await
            .expect("skip should return Ok");
        assert!(matches!(
            outcome,
            CreateSnapshotAndPruneOutcome::SkippedAlreadyInFlight
        ));
        assert!(manager
            .get_metadata(100)
            .await
            .expect("get metadata")
            .is_none());
        manager.set_build_in_flight_for_test(false);
    }
}
