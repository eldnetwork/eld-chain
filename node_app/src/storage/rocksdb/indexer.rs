use crate::indexer::{IndexedEvent, IndexedTransaction, TransactionStatus};
use crate::storage::traits::TransactionIndexerStorage;
use abci::types::Event;
use eld_common::address::Address;
use eld_common::cado::{
    epoch_record_path_name, epochs_fully_inside_window, CadoPath, CadoPathKey, CadoType,
};
use eld_common::error::EldError;
use eld_common::tx::{HasSender, PayloadInner, Tx};
use hex;
use rocksdb::{ColumnFamily, Direction, IteratorMode, ReadOptions, Transaction, TransactionDB};
use serde_json;
use sha2::{Digest, Sha256};
use tracing::debug;

use super::keys::{
    IndexedTxChronFilters, BLOCK_POS_CHRON_LOWER_BOUND,
    INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY, VERIFIED_PROOF_GLOBAL_REWARDS_COUNT_KEY,
};
use super::RocksDBStorage;

fn indexed_event_from_abci(
    tx_id: &str,
    event_index: u32,
    event: &Event,
    block_height: u64,
    block_index: u32,
    timestamp: u64,
) -> IndexedEvent {
    let attributes = event
        .attributes
        .iter()
        .map(|attr| {
            (
                String::from_utf8_lossy(&attr.key).to_string(),
                String::from_utf8_lossy(&attr.value).to_string(),
            )
        })
        .collect();
    IndexedEvent {
        tx_id: tx_id.to_string(),
        event_index,
        event_type: event.r#type.clone(),
        attributes,
        block_height,
        block_index,
        timestamp,
    }
}

impl RocksDBStorage {
    /// Count primary indexed-transaction rows (same predicate as listing endpoints).
    pub(super) fn count_primary_indexed_transaction_keys(
        &self,
        cf: &ColumnFamily,
    ) -> Result<u64, EldError> {
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let mut n = 0u64;
        for item in iter {
            let (key, _) = item.map_err(|e| EldError::StorageError {
                operation: "count_primary_indexed_transactions".to_string(),
                details: format!("Failed to iterate indexed_transactions column family: {e}"),
            })?;
            let key_str = String::from_utf8_lossy(&key);
            if key_str.starts_with("0x") && !key_str.contains(':') {
                n += 1;
            }
        }
        Ok(n)
    }

    pub(super) fn decode_u64_meta(bytes: &[u8]) -> Result<u64, EldError> {
        let arr: [u8; 8] = bytes.try_into().map_err(|_| EldError::StorageError {
            operation: "decode_indexed_transaction_total_count_meta".to_string(),
            details: format!(
                "Invalid indexed transaction total meta length (expected 8, got {})",
                bytes.len()
            ),
        })?;
        Ok(u64::from_le_bytes(arr))
    }

    /// Current total count of primary indexed transactions (for API pagination).
    pub(super) fn indexed_transaction_primary_total_count(&self) -> Result<u64, EldError> {
        let cf = self.indexed_transactions_cf()?;
        match self
            .db
            .get_cf(cf, INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY)
            .map_err(|e| EldError::StorageError {
                operation: "get_indexed_transaction_total_count_meta".to_string(),
                details: format!("Failed to read indexed transaction total meta: {e}"),
            })? {
            Some(bytes) => Self::decode_u64_meta(&bytes),
            None => {
                let n = self.count_primary_indexed_transaction_keys(cf)?;
                self.db
                    .put_cf(
                        cf,
                        INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY,
                        n.to_le_bytes(),
                    )
                    .map_err(|e| EldError::StorageError {
                        operation: "put_indexed_transaction_total_count_meta".to_string(),
                        details: format!("Failed to repair indexed transaction total meta: {e}"),
                    })?;
                Ok(n)
            }
        }
    }

    pub(super) fn bump_indexed_transaction_primary_total_count(&self) -> Result<(), EldError> {
        let _guard = self
            .indexed_transaction_primary_total_mutex
            .lock()
            .map_err(|_| EldError::StorageError {
                operation: "indexed_transaction_primary_total_mutex".to_string(),
                details: "Mutex poisoned while updating indexed transaction total count"
                    .to_string(),
            })?;
        let cf = self.indexed_transactions_cf()?;
        let new_total = match self
            .db
            .get_cf(cf, INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY)
            .map_err(|e| EldError::StorageError {
                operation: "get_indexed_transaction_total_count_meta".to_string(),
                details: format!("Failed to read indexed transaction total meta: {e}"),
            })? {
            Some(bytes) => {
                let cur = Self::decode_u64_meta(&bytes)?;
                cur.checked_add(1).ok_or_else(|| EldError::StorageError {
                    operation: "bump_indexed_transaction_total_count".to_string(),
                    details: "Indexed transaction total count overflow".to_string(),
                })?
            }
            None => self.count_primary_indexed_transaction_keys(cf)?,
        };

        self.db
            .put_cf(
                cf,
                INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY,
                new_total.to_le_bytes(),
            )
            .map_err(|e| EldError::StorageError {
                operation: "put_indexed_transaction_total_count_meta".to_string(),
                details: format!("Failed to write indexed transaction total meta: {e}"),
            })?;
        Ok(())
    }

    pub(super) fn read_verified_proof_global_rewards_count(
        &self,
        cf: &ColumnFamily,
    ) -> Result<u64, EldError> {
        match self
            .db
            .get_cf(cf, VERIFIED_PROOF_GLOBAL_REWARDS_COUNT_KEY)
            .map_err(|e| EldError::StorageError {
                operation: "get_verified_proof_global_rewards_count".to_string(),
                details: e.to_string(),
            })? {
            None => Ok(0),
            Some(bytes) => {
                if bytes.len() == 8 {
                    let arr: [u8; 8] =
                        bytes
                            .as_slice()
                            .try_into()
                            .map_err(|_| EldError::StorageError {
                                operation: "verified_proof_global_rewards_count_slice".to_string(),
                                details: "invalid length".to_string(),
                            })?;
                    Ok(u64::from_le_bytes(arr))
                } else {
                    Ok(0)
                }
            }
        }
    }

    pub(super) fn bump_verified_proof_global_rewards_count(
        &self,
        cf: &ColumnFamily,
    ) -> Result<(), EldError> {
        let next = self
            .read_verified_proof_global_rewards_count(cf)?
            .saturating_add(1);
        self.db
            .put_cf(
                cf,
                VERIFIED_PROOF_GLOBAL_REWARDS_COUNT_KEY,
                next.to_le_bytes(),
            )
            .map_err(|e| EldError::StorageError {
                operation: "put_verified_proof_global_rewards_count".to_string(),
                details: e.to_string(),
            })?;
        Ok(())
    }
    pub(super) fn indexed_transaction_matches_optional_filters(
        ix: &IndexedTransaction,
        block_height_filter: Option<u64>,
        sender_filter: Option<Address>,
        payload_type_filter: Option<&str>,
    ) -> bool {
        use eld_common::tx::HasSender;
        if let Some(h) = block_height_filter {
            if ix.block_height != h {
                return false;
            }
        }
        if let Some(addr) = sender_filter {
            if ix.tx.payload.inner.sender() != addr {
                return false;
            }
        }
        if let Some(pt) = payload_type_filter {
            if ix.tx.payload.r#type != pt {
                return false;
            }
        }
        true
    }

    pub(super) fn load_primary_indexed_transaction_by_cf(
        cf: &ColumnFamily,
        db: &TransactionDB,
        tx_id_bytes: &[u8],
    ) -> Result<IndexedTransaction, EldError> {
        let normalized = std::str::from_utf8(tx_id_bytes).map_err(|e| EldError::StorageError {
            operation: "indexed_tx_secondary_tx_id_utf8".to_string(),
            details: format!("{e}"),
        })?;
        let normalized = normalized.to_string();

        let data = db
            .get_cf(cf, normalized.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "get_indexed_transaction_by_secondary".to_string(),
                details: format!("{e}"),
            })?;
        match data {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| EldError::StorageError {
                operation: "deserialize_indexed_transaction_by_secondary".to_string(),
                details: format!("{e}"),
            }),
            None => Err(EldError::StorageError {
                operation: "missing_primary_for_block_pos_row".to_string(),
                details: format!("Missing primary IndexedTransaction row for {normalized}"),
            }),
        }
    }

    pub(crate) fn list_indexed_transactions_chron_desc_scanned(
        &self,
        cursor_tuple: Option<(u64, u32)>,
        filters: IndexedTxChronFilters<'_>,
        fetch_limit: usize,
    ) -> Result<Vec<IndexedTransaction>, EldError> {
        let cf = self.indexed_transactions_cf()?;
        let readopts = Self::chron_block_position_read_options();

        let seek_bytes_owned =
            cursor_tuple.map(|(ch, ci)| Self::block_position_chron_index_key_bytes(ch, ci));

        let mode = match &seek_bytes_owned {
            None => IteratorMode::End,
            Some(ck_vec) => IteratorMode::From(ck_vec.as_slice(), Direction::Reverse),
        };

        let iter = self.db.iterator_cf_opt(cf, readopts, mode);

        let mut out = Vec::new();

        for res in iter {
            let (key, value) = res.map_err(|e| EldError::StorageError {
                operation: "iterate_block_pos_tx_chron_desc".to_string(),
                details: format!("{e}"),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| EldError::StorageError {
                operation: "block_pos_index_key_utf8".to_string(),
                details: format!("{e}"),
            })?;

            if !key_str.starts_with("block_pos:") {
                continue;
            }

            let (kh, ki) = Self::parse_block_position_chron_key(key_str).ok_or_else(|| {
                EldError::StorageError {
                    operation: "parse_block_pos_key".to_string(),
                    details: format!("Malformed block_pos key: {key_str}"),
                }
            })?;

            if let Some((ch, ci)) = cursor_tuple {
                // Skip txs newer than or equal to the cursor anchor (exclusive next page boundary).
                if kh > ch || (kh == ch && ki >= ci) {
                    continue;
                }
            }

            let indexed_tx = Self::load_primary_indexed_transaction_by_cf(cf, &self.db, &value)?;
            if !Self::indexed_transaction_matches_optional_filters(
                &indexed_tx,
                filters.block_height,
                filters.sender,
                filters.payload_type,
            ) {
                continue;
            }

            out.push(indexed_tx);
            if out.len() >= fetch_limit {
                break;
            }
        }

        Ok(out)
    }

    pub(crate) fn count_indexed_transactions_matching_chron_filters(
        &self,
        filters: IndexedTxChronFilters<'_>,
    ) -> Result<u64, EldError> {
        let cf = self.indexed_transactions_cf()?;
        let readopts = Self::chron_block_position_read_options();
        let iter = self.db.iterator_cf_opt(
            cf,
            readopts,
            IteratorMode::From(BLOCK_POS_CHRON_LOWER_BOUND, Direction::Forward),
        );

        let mut n = 0u64;

        for res in iter {
            let (_, value) = res.map_err(|e| EldError::StorageError {
                operation: "count_filtered_chron_scan".to_string(),
                details: format!("{e}"),
            })?;

            let indexed_tx = Self::load_primary_indexed_transaction_by_cf(cf, &self.db, &value)?;
            if Self::indexed_transaction_matches_optional_filters(
                &indexed_tx,
                filters.block_height,
                filters.sender,
                filters.payload_type,
            ) {
                n += 1;
            }
        }

        Ok(n)
    }

    /// Delete per-epoch CADO rows whose whole span sits in `start_exclusive < h < end_exclusive`.
    ///
    /// The `LATEST` alias is not a numbered epoch path, so it is left in place.
    fn prune_epoch_records_between(
        &self,
        start_exclusive: i64,
        end_exclusive: i64,
        blocks_per_epoch: i64,
    ) -> Result<(), EldError> {
        let Some((first, last)) =
            epochs_fully_inside_window(start_exclusive, end_exclusive, blocks_per_epoch)
        else {
            return Ok(());
        };

        for epoch in first..=last {
            let name = epoch_record_path_name(epoch)?;
            let path = CadoPath::new(CadoType::EpochRecord, CadoPathKey::Name(&name))?;
            if self.get_cado_by_path(path.clone())?.is_none() {
                continue;
            }
            self.system_delete_cado(path, &epoch.to_string())?;
        }
        Ok(())
    }
}

/// One `block_pos:` hit collected before the prune write transaction.
struct PruneRow {
    chron_key: Vec<u8>,
    indexed: Option<IndexedTransaction>,
    events: Vec<IndexedEvent>,
    had_vp_index: bool,
}

impl RocksDBStorage {
    fn collect_prune_rows(
        &self,
        tx_cf: &ColumnFamily,
        start_exclusive: u64,
        end_exclusive: u64,
        lower: Vec<u8>,
        readopts: ReadOptions,
    ) -> Result<Vec<PruneRow>, EldError> {
        let iter = self.db.iterator_cf_opt(
            tx_cf,
            readopts,
            IteratorMode::From(lower.as_slice(), Direction::Forward),
        );

        let mut rows = Vec::new();
        for res in iter {
            let (key, value) = res.map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_iterate".to_string(),
                details: format!("Failed to iterate block_pos keys: {e}"),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_key_utf8".to_string(),
                details: format!("{e}"),
            })?;
            if !key_str.starts_with("block_pos:") {
                continue;
            }
            let (height, _) = Self::parse_block_position_chron_key(key_str).ok_or_else(|| {
                EldError::StorageError {
                    operation: "prune_indexed_data_between_parse_block_pos".to_string(),
                    details: format!("Malformed block_pos key: {key_str}"),
                }
            })?;
            if height <= start_exclusive || height >= end_exclusive {
                continue;
            }

            let indexed =
                match Self::load_primary_indexed_transaction_by_cf(tx_cf, &self.db, &value) {
                    Ok(ix) => Some(ix),
                    Err(EldError::StorageError {
                        operation,
                        details: _,
                    }) if operation == "missing_primary_for_block_pos_row" => None,
                    Err(e) => return Err(e),
                };

            let (events, had_vp_index) = self.load_events_and_vp_flag(tx_cf, &indexed)?;
            rows.push(PruneRow {
                chron_key: key.to_vec(),
                indexed,
                events,
                had_vp_index,
            });
        }
        Ok(rows)
    }

    fn load_events_and_vp_flag(
        &self,
        tx_cf: &ColumnFamily,
        indexed: &Option<IndexedTransaction>,
    ) -> Result<(Vec<IndexedEvent>, bool), EldError> {
        let Some(ix) = indexed else {
            return Ok((Vec::new(), false));
        };

        let mut events = self.get_events_by_tx_id(&ix.id)?;
        if events.is_empty() && !ix.events.is_empty() {
            events = ix.events.clone();
        }
        let had_vp = self.had_verified_proof_index(tx_cf, ix)?;
        Ok((events, had_vp))
    }

    fn had_verified_proof_index(
        &self,
        tx_cf: &ColumnFamily,
        ix: &IndexedTransaction,
    ) -> Result<bool, EldError> {
        if ix.status != TransactionStatus::Success {
            return Ok(false);
        }
        let PayloadInner::VerifiedProof(vp_tx) = &ix.tx.payload.inner else {
            return Ok(false);
        };
        if vp_tx.failed {
            return Ok(false);
        }

        let provider_key = vp_tx.capacity_provider.to_string().to_lowercase();
        let vp_key =
            Self::verified_proof_reward_index_key(&provider_key, ix.block_height, ix.block_index);
        Ok(self
            .db
            .get_cf(tx_cf, &vp_key)
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_get_vp".to_string(),
                details: e.to_string(),
            })?
            .is_some())
    }

    fn delete_prune_row(
        &self,
        txn: &Transaction<'_, TransactionDB>,
        tx_cf: &ColumnFamily,
        events_cf: &ColumnFamily,
        row: &PruneRow,
    ) -> Result<(), EldError> {
        // Chronological secondary: `block_pos:{height:016x}:{index:08x}` → tx_id.
        txn.delete_cf(tx_cf, &row.chron_key)
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_delete_block_pos".to_string(),
                details: e.to_string(),
            })?;

        let Some(ix) = &row.indexed else {
            return Ok(());
        };

        // Primary body: `0x…` tx_id → IndexedTransaction JSON.
        txn.delete_cf(tx_cf, ix.id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_delete_primary".to_string(),
                details: e.to_string(),
            })?;

        // Legacy block secondary: `block_height:{h}:tx_index:{i}` → tx_id.
        let legacy_block_key = format!(
            "block_height:{}:tx_index:{}",
            ix.block_height, ix.block_index
        );
        txn.delete_cf(tx_cf, legacy_block_key.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_delete_legacy_block".to_string(),
                details: e.to_string(),
            })?;

        // Sender secondary: `sender:{address}:nonce:{nonce}` → tx_id.
        let sender = ix.tx.payload.inner.sender();
        let sender_key = format!("sender:{}:nonce:{}", sender, ix.tx.nonce.value());
        txn.delete_cf(tx_cf, sender_key.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_delete_sender".to_string(),
                details: e.to_string(),
            })?;

        // Payload-type secondary: `type:{payload_type}:{tx_id}` → tx_id.
        let type_key = format!("type:{}:{}", ix.tx.payload.r#type, ix.id);
        txn.delete_cf(tx_cf, type_key.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_delete_type".to_string(),
                details: e.to_string(),
            })?;

        // Verified-proof reward secondary: `vp:{provider}:{height:016x}:{index:08x}`.
        if row.had_vp_index {
            if let PayloadInner::VerifiedProof(vp_tx) = &ix.tx.payload.inner {
                let provider_key = vp_tx.capacity_provider.to_string().to_lowercase();
                let vp_key = Self::verified_proof_reward_index_key(
                    &provider_key,
                    ix.block_height,
                    ix.block_index,
                );
                txn.delete_cf(tx_cf, &vp_key)
                    .map_err(|e| EldError::StorageError {
                        operation: "prune_indexed_data_between_delete_vp".to_string(),
                        details: e.to_string(),
                    })?;
            }
        }

        for ev in &row.events {
            let primary_key = format!("tx_id:{}:event_index:{}", ev.tx_id, ev.event_index);
            let type_index_key = format!(
                "type:{}:block_height:{}:block_index:{}:event_index:{}",
                ev.event_type, ev.block_height, ev.block_index, ev.event_index
            );
            let block_event_key = format!(
                "block_height:{}:block_index:{}:event_index:{}",
                ev.block_height, ev.block_index, ev.event_index
            );
            // Event primary: `tx_id:{tx_id}:event_index:{ei}` → IndexedEvent JSON.
            txn.delete_cf(events_cf, primary_key.as_bytes())
                .map_err(|e| EldError::StorageError {
                    operation: "prune_indexed_data_between_delete_event_primary".to_string(),
                    details: e.to_string(),
                })?;
            // Event type secondary:
            // `type:{event_type}:block_height:{h}:block_index:{i}:event_index:{ei}`.
            txn.delete_cf(events_cf, type_index_key.as_bytes())
                .map_err(|e| EldError::StorageError {
                    operation: "prune_indexed_data_between_delete_event_type".to_string(),
                    details: e.to_string(),
                })?;
            // Event block secondary:
            // `block_height:{h}:block_index:{i}:event_index:{ei}`.
            txn.delete_cf(events_cf, block_event_key.as_bytes())
                .map_err(|e| EldError::StorageError {
                    operation: "prune_indexed_data_between_delete_event_block".to_string(),
                    details: e.to_string(),
                })?;
        }

        Ok(())
    }
}

impl TransactionIndexerStorage for RocksDBStorage {
    /// Calculate transaction ID from transaction
    fn calculate_tx_id(&self, tx: &Tx) -> Result<String, EldError> {
        // Serialize transaction to JSON (canonical form with signature)
        // Using JSON instead of bincode because transactions contain variable-length sequences
        // that bincode's default config can't handle
        let serialized = serde_json::to_vec(tx).map_err(|e| EldError::StorageError {
            operation: "calculate_tx_id".to_string(),
            details: format!("Failed to serialize transaction: {e}"),
        })?;
        let hash = Sha256::digest(&serialized);
        Ok(format!("0x{}", hex::encode(hash)))
    }

    /// Index a transaction with all secondary indexes
    fn index_transaction(
        &self,
        tx: &Tx,
        block_height: u64,
        block_index: u32,
        status: TransactionStatus,
        gas_used: Option<u64>,
        events: &[Event],
    ) -> Result<(), EldError> {
        let tx_id = self.calculate_tx_id(tx)?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| EldError::StorageError {
                operation: "get_timestamp".to_string(),
                details: format!("Failed to get timestamp: {e}"),
            })?
            .as_secs();

        let events = events
            .iter()
            .enumerate()
            .map(|(event_index, event)| {
                indexed_event_from_abci(
                    &tx_id,
                    event_index as u32,
                    event,
                    block_height,
                    block_index,
                    timestamp,
                )
            })
            .collect();

        let indexed_tx = IndexedTransaction {
            id: tx_id.clone(),
            block_height,
            block_index,
            timestamp,
            tx: tx.clone(),
            status,
            gas_used,
            events,
        };

        // Use JSON serialization since IndexedTransaction contains Tx which has variable-length sequences
        let serialized = serde_json::to_vec(&indexed_tx).map_err(|e| EldError::StorageError {
            operation: "serialize_indexed_transaction".to_string(),
            details: format!("Failed to serialize indexed transaction: {e}"),
        })?;

        let cf = self.indexed_transactions_cf()?;

        let primary_already_present = self
            .db
            .get_cf(cf, tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "get_indexed_transaction_primary_exists".to_string(),
                details: format!("Failed to check indexed transaction existence: {e}"),
            })?
            .is_some();

        // Store primary key: tx_id -> IndexedTransaction
        self.db
            .put_cf(cf, tx_id.as_bytes(), &serialized)
            .map_err(|e| EldError::StorageError {
                operation: "put_indexed_transaction".to_string(),
                details: format!("Failed to store indexed transaction: {e}"),
            })?;

        // Store secondary index: block_height:tx_index -> tx_id
        let block_index_key = format!("block_height:{block_height}:tx_index:{block_index}");
        self.db
            .put_cf(cf, block_index_key.as_bytes(), tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_block_index_key".to_string(),
                details: format!("Failed to store block index key: {e}"),
            })?;

        let chron_key = Self::block_position_chron_index_key_bytes(block_height, block_index);
        self.db
            .put_cf(cf, &chron_key, tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_block_pos_chron_index_key".to_string(),
                details: format!("Failed to store block_pos chronological index: {e}"),
            })?;

        // Store secondary index: sender:nonce -> tx_id
        let sender = tx.payload.inner.sender();
        let sender_index_key = format!("sender:{}:nonce:{}", sender, tx.nonce.value());
        self.db
            .put_cf(cf, sender_index_key.as_bytes(), tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_sender_index_key".to_string(),
                details: format!("Failed to store sender index key: {e}"),
            })?;

        // Store secondary index: type:payload_type:tx_id -> tx_id
        let payload_type = &tx.payload.r#type;
        let type_index_key = format!("type:{payload_type}:{tx_id}");
        self.db
            .put_cf(cf, type_index_key.as_bytes(), tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_type_index_key".to_string(),
                details: format!("Failed to store type index key: {e}"),
            })?;

        if status == TransactionStatus::Success {
            if let PayloadInner::VerifiedProof(vp_tx) = &tx.payload.inner {
                if !vp_tx.failed {
                    let provider_key = vp_tx.capacity_provider.to_string().to_lowercase();
                    let vp_key = Self::verified_proof_reward_index_key(
                        &provider_key,
                        block_height,
                        block_index,
                    );
                    self.db
                        .put_cf(cf, &vp_key, [])
                        .map_err(|e| EldError::StorageError {
                            operation: "put_verified_proof_reward_index".to_string(),
                            details: format!("Failed to store verified proof reward index: {e}"),
                        })?;
                    self.bump_verified_proof_global_rewards_count(cf)?;
                }
            }
        }

        debug!(
            tx_id = %tx_id,
            block_height = block_height,
            block_index = block_index,
            "Indexed transaction"
        );

        if !primary_already_present {
            self.bump_indexed_transaction_primary_total_count()?;
        }

        Ok(())
    }

    /// Get a transaction by ID
    fn get_indexed_transaction(&self, tx_id: &str) -> Result<Option<IndexedTransaction>, EldError> {
        let cf = self.indexed_transactions_cf()?;

        // Normalize tx_id: ensure it has "0x" prefix to match how it's stored
        // Transactions are stored with the full "0x" prefix from calculate_tx_id
        let normalized_tx_id = if tx_id.starts_with("0x") {
            tx_id.to_string()
        } else {
            // If no prefix, add it (though API should always provide it)
            format!("0x{tx_id}")
        };

        let value = self
            .db
            .get_cf(cf, normalized_tx_id.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "get_indexed_transaction".to_string(),
                details: format!("Failed to get indexed transaction: {e}"),
            })?;

        match value {
            Some(data) => {
                // Use JSON deserialization since IndexedTransaction is stored as JSON
                let indexed_tx: IndexedTransaction =
                    serde_json::from_slice(&data).map_err(|e| EldError::StorageError {
                        operation: "deserialize_indexed_transaction".to_string(),
                        details: format!("Failed to deserialize indexed transaction: {e}"),
                    })?;
                Ok(Some(indexed_tx))
            }
            None => Ok(None),
        }
    }

    fn get_indexed_transactions_primary_total(&self) -> Result<u64, EldError> {
        self.indexed_transaction_primary_total_count()
    }

    fn list_indexed_transactions_chron_desc(
        &self,
        cursor_exclusive: Option<(u64, u32)>,
        limit: u32,
        fetch_one_extra_row: bool,
        block_height: Option<u64>,
        sender: Option<Address>,
        payload_type: Option<&str>,
    ) -> Result<Vec<IndexedTransaction>, EldError> {
        let extra = u32::from(fetch_one_extra_row);
        let fetch_limit = (limit.saturating_add(extra) as usize).max(1);
        self.list_indexed_transactions_chron_desc_scanned(
            cursor_exclusive,
            IndexedTxChronFilters {
                block_height,
                sender,
                payload_type,
            },
            fetch_limit,
        )
    }

    fn count_indexed_transactions_matching_filters_chron_scan(
        &self,
        block_height: Option<u64>,
        sender: Option<Address>,
        payload_type: Option<&str>,
    ) -> Result<u64, EldError> {
        self.count_indexed_transactions_matching_chron_filters(IndexedTxChronFilters {
            block_height,
            sender,
            payload_type,
        })
    }

    /// Index an event with all secondary indexes
    fn index_event(
        &self,
        tx_id: &str,
        event_index: u32,
        event: &Event,
        block_height: u64,
        block_index: u32,
    ) -> Result<(), EldError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| EldError::StorageError {
                operation: "get_timestamp".to_string(),
                details: format!("Failed to get timestamp: {e}"),
            })?
            .as_secs();

        let indexed_event = indexed_event_from_abci(
            tx_id,
            event_index,
            event,
            block_height,
            block_index,
            timestamp,
        );
        let event_type = indexed_event.event_type.clone();

        // Use JSON serialization (same as transactions)
        let serialized =
            serde_json::to_vec(&indexed_event).map_err(|e| EldError::StorageError {
                operation: "serialize_indexed_event".to_string(),
                details: format!("Failed to serialize indexed event: {e}"),
            })?;

        let cf = self.indexed_events_cf()?;

        // Store primary key: tx_id:event_index -> IndexedEvent
        let primary_key = format!("tx_id:{tx_id}:event_index:{event_index}");
        self.db
            .put_cf(cf, primary_key.as_bytes(), &serialized)
            .map_err(|e| EldError::StorageError {
                operation: "put_indexed_event".to_string(),
                details: format!("Failed to store indexed event: {e}"),
            })?;

        // Store secondary index: type:event_type:block_height:block_index:event_index -> tx_id:event_index
        let type_index_key = format!(
            "type:{event_type}:block_height:{block_height}:block_index:{block_index}:event_index:{event_index}"
        );
        self.db
            .put_cf(cf, type_index_key.as_bytes(), primary_key.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_type_index_key".to_string(),
                details: format!("Failed to store type index key: {e}"),
            })?;

        // Store secondary index: block_height:block_index:event_index -> tx_id:event_index
        let block_index_key = format!(
            "block_height:{block_height}:block_index:{block_index}:event_index:{event_index}"
        );
        self.db
            .put_cf(cf, block_index_key.as_bytes(), primary_key.as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "put_block_index_key".to_string(),
                details: format!("Failed to store block index key: {e}"),
            })?;

        debug!(
            tx_id = %tx_id,
            event_index = event_index,
            event_type = %event_type,
            block_height = block_height,
            block_index = block_index,
            "Indexed event"
        );

        Ok(())
    }

    /// Get all events for a transaction
    fn get_events_by_tx_id(&self, tx_id: &str) -> Result<Vec<IndexedEvent>, EldError> {
        let cf = self.indexed_events_cf()?;

        // Normalize tx_id: ensure it has "0x" prefix to match how it's stored
        let normalized_tx_id = if tx_id.starts_with("0x") {
            tx_id.to_string()
        } else {
            format!("0x{tx_id}")
        };

        let prefix = format!("tx_id:{normalized_tx_id}:event_index:");
        let mut events = Vec::new();

        // Iterate through all keys starting from the beginning and filter by prefix
        // This is more reliable than IteratorMode::From for prefix matching
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) = item.map_err(|e| EldError::StorageError {
                operation: "iterate_events_by_tx_id".to_string(),
                details: format!("Failed to iterate events: {e}"),
            })?;

            let key_str = String::from_utf8_lossy(&key);

            // Only process keys that start with our prefix
            if key_str.starts_with(&prefix) {
                // Deserialize the event
                let indexed_event: IndexedEvent =
                    serde_json::from_slice(&value).map_err(|e| EldError::StorageError {
                        operation: "deserialize_indexed_event".to_string(),
                        details: format!("Failed to deserialize indexed event: {e}"),
                    })?;
                events.push(indexed_event);
            } else if key_str.as_ref() > prefix.as_str() {
                // Since keys are sorted, if we've passed the prefix, we can stop
                // This optimization only works if the prefix is a valid key prefix
                // For our case, keys are sorted lexicographically, so this is safe
                break;
            }
        }

        // Sort by event_index to ensure correct order
        events.sort_by_key(|e| e.event_index);

        Ok(events)
    }

    /// Get all events (paginated, descending chronological order)
    fn get_all_events(&self, page: u32, limit: u32) -> Result<(Vec<IndexedEvent>, u64), EldError> {
        let cf = self.indexed_events_cf()?;
        let offset = ((page - 1) * limit) as u64;

        let prefix = "block_height:";
        let mut events = Vec::new();
        let mut count = 0u64;
        let mut skipped = 0u64;

        // Use reverse iteration to get events in descending chronological order (newest first)
        // Start from the end and iterate backwards through block_height keys
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::End);

        for item in iter {
            let (key, value) = item.map_err(|e| EldError::StorageError {
                operation: "iterate_all_events".to_string(),
                details: format!("Failed to iterate events: {e}"),
            })?;

            let key_str = String::from_utf8_lossy(&key);

            // Only process block_height index keys (secondary index for chronological ordering)
            if key_str.starts_with(prefix)
                && key_str.contains(":block_index:")
                && key_str.contains(":event_index:")
            {
                count += 1;

                if skipped < offset {
                    skipped += 1;
                    continue;
                }

                if events.len() < limit as usize {
                    // Get the primary key reference
                    let primary_key = String::from_utf8_lossy(&value);

                    // Fetch the actual event data using the primary key
                    let event_data = self.db.get_cf(cf, primary_key.as_bytes()).map_err(|e| {
                        EldError::StorageError {
                            operation: "get_event_by_primary_key".to_string(),
                            details: format!("Failed to get event data: {e}"),
                        }
                    })?;

                    if let Some(data) = event_data {
                        let indexed_event: IndexedEvent =
                            serde_json::from_slice(&data).map_err(|e| EldError::StorageError {
                                operation: "deserialize_indexed_event".to_string(),
                                details: format!("Failed to deserialize indexed event: {e}"),
                            })?;
                        events.push(indexed_event);
                    }
                }
            }
        }

        Ok((events, count))
    }

    fn aggregate_verified_proof_rewards(
        &self,
        provider: &str,
        from_height: u64,
        to_height: u64,
        reward_per_proof: u128,
    ) -> Result<(u64, u128), EldError> {
        let addr = Address::parse_hex_str(provider)?;
        let normalized = addr.hex_with_prefix().to_lowercase();
        let lower = Self::verified_proof_reward_index_key(&normalized, from_height, 0);
        let prefix = format!("vp:{normalized}:").into_bytes();
        let cf = self.indexed_transactions_cf()?;
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(lower.as_slice(), Direction::Forward));
        let mut count = 0u64;
        for item in iter {
            let (key, _) = item.map_err(|e| EldError::StorageError {
                operation: "aggregate_verified_proof_rewards_iterate".to_string(),
                details: e.to_string(),
            })?;
            if !key.starts_with(&prefix) {
                break;
            }
            let key_str = String::from_utf8_lossy(&key);
            let Some((_addr_key, h, _bi)) = Self::parse_verified_proof_reward_index_key(&key_str)
            else {
                continue;
            };
            if h < from_height {
                continue;
            }
            if h > to_height {
                break;
            }
            count += 1;
        }
        let total = (count as u128).saturating_mul(reward_per_proof);
        Ok((count, total))
    }

    fn global_verified_proof_rewards(
        &self,
        reward_per_proof: u128,
    ) -> Result<(u64, u128), EldError> {
        let cf = self.indexed_transactions_cf()?;
        let count = self.read_verified_proof_global_rewards_count(cf)?;
        let total = (count as u128).saturating_mul(reward_per_proof);
        Ok((count, total))
    }

    fn get_indexer_cursor(&self) -> Result<Option<u64>, EldError> {
        const KEY: &[u8] = b"meta:indexer:last_synced_height";
        let cf = self.indexed_transactions_cf()?;
        let Some(bytes) = self
            .db
            .get_cf(cf, KEY)
            .map_err(|e| EldError::StorageError {
                operation: "get_indexer_cursor".to_string(),
                details: format!("Failed to read indexer cursor: {e}"),
            })?
        else {
            return Ok(None);
        };
        let s = std::str::from_utf8(&bytes).map_err(|e| EldError::StorageError {
            operation: "get_indexer_cursor".to_string(),
            details: format!("Invalid UTF-8 in indexer cursor: {e}"),
        })?;
        s.parse::<u64>()
            .map(Some)
            .map_err(|e| EldError::StorageError {
                operation: "get_indexer_cursor".to_string(),
                details: format!("Invalid indexer cursor value {s:?}: {e}"),
            })
    }

    fn set_indexer_cursor(&self, height: u64) -> Result<(), EldError> {
        const KEY: &[u8] = b"meta:indexer:last_synced_height";
        let cf = self.indexed_transactions_cf()?;
        self.db
            .put_cf(cf, KEY, height.to_string().as_bytes())
            .map_err(|e| EldError::StorageError {
                operation: "set_indexer_cursor".to_string(),
                details: format!("Failed to write indexer cursor: {e}"),
            })
    }

    fn block_position_indexed(
        &self,
        block_height: u64,
        block_index: u32,
    ) -> Result<bool, EldError> {
        let cf = self.indexed_transactions_cf()?;
        let key = Self::block_position_chron_index_key_bytes(block_height, block_index);
        self.db
            .get_cf(cf, &key)
            .map(|opt| opt.is_some())
            .map_err(|e| EldError::StorageError {
                operation: "block_position_indexed".to_string(),
                details: format!(
                    "Failed to read block_pos index at {block_height}:{block_index}: {e}"
                ),
            })
    }

    fn prune_indexed_data_between(
        &self,
        start_exclusive: u64,
        end_exclusive: u64,
    ) -> Result<(), EldError> {
        // Heights deleted are start_exclusive < h < end_exclusive.
        let Some(first_height) = start_exclusive.checked_add(1) else {
            return Ok(());
        };
        if first_height >= end_exclusive {
            return Ok(());
        }

        if let (Ok(start_i), Ok(end_i)) =
            (i64::try_from(start_exclusive), i64::try_from(end_exclusive))
        {
            let blocks_per_epoch = self.get_protocol_constants().blocks_per_epoch;
            self.prune_epoch_records_between(start_i, end_i, blocks_per_epoch)?;
        }

        let lower = Self::block_position_chron_index_key_bytes(first_height, 0);
        let upper = Self::block_position_chron_index_key_bytes(end_exclusive, 0);

        let mut readopts = ReadOptions::default();
        readopts.set_iterate_lower_bound(lower.clone());
        readopts.set_iterate_upper_bound(upper);
        readopts.set_total_order_seek(true);

        let tx_cf = self.indexed_transactions_cf()?;
        let events_cf = self.indexed_events_cf()?;

        let rows =
            self.collect_prune_rows(tx_cf, start_exclusive, end_exclusive, lower, readopts)?;
        if rows.is_empty() {
            return Ok(());
        }

        // Hold the primary-total mutex for the rest of this function (RAII). The guard
        // is not read; dropping it at the end unlocks. Same serialization as
        // bump_indexed_transaction_primary_total_count against the sync thread.
        let _guard = self
            .indexed_transaction_primary_total_mutex
            .lock()
            .map_err(|_| EldError::StorageError {
                operation: "prune_indexed_data_between_mutex".to_string(),
                details: "Mutex poisoned while pruning indexed transactions".to_string(),
            })?;

        let removed_primary = rows.iter().filter(|r| r.indexed.is_some()).count() as u64;
        let removed_vp = rows.iter().filter(|r| r.had_vp_index).count() as u64;

        let new_primary = self
            .indexed_transaction_primary_total_count()?
            .saturating_sub(removed_primary);
        let new_vp = self
            .read_verified_proof_global_rewards_count(tx_cf)?
            .saturating_sub(removed_vp);

        let txn = self.begin_transaction();
        for row in &rows {
            self.delete_prune_row(&txn, tx_cf, events_cf, row)?;
        }

        // Meta: `_meta:indexed_transactions_primary_total` after removing primary bodies.
        if removed_primary > 0 {
            txn.put_cf(
                tx_cf,
                INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY,
                new_primary.to_le_bytes(),
            )
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_put_primary_total".to_string(),
                details: e.to_string(),
            })?;
        }
        // Meta: `_meta:verified_proof_global_rewards_count` after removing `vp:` rows.
        if removed_vp > 0 {
            txn.put_cf(
                tx_cf,
                VERIFIED_PROOF_GLOBAL_REWARDS_COUNT_KEY,
                new_vp.to_le_bytes(),
            )
            .map_err(|e| EldError::StorageError {
                operation: "prune_indexed_data_between_put_vp_total".to_string(),
                details: e.to_string(),
            })?;
        }

        txn.commit().map_err(|e| EldError::StorageError {
            operation: "prune_indexed_data_between_commit".to_string(),
            details: e.to_string(),
        })?;

        Ok(())
    }
}
