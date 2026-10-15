//! Read-only inventory of an Eld ABCI RocksDB directory.

use crate::model::{CadoTypeCount, CfSstBytes, RocksDbInventory};
use crate::scan::SCAN_BUDGET;
use eld_common::constants::cado::{
    PATH_PREFIX_ACCOUNT, PATH_PREFIX_APP_STATE_SNAPSHOT, PATH_PREFIX_APP_STATE_TIP,
    PATH_PREFIX_CADO_MAP, PATH_PREFIX_CHUNK_REFERENCE, PATH_PREFIX_EPOCH_RECORD,
    PATH_PREFIX_NAMESPACE_REGISTRY, PATH_PREFIX_PINBOARD, PATH_PREFIX_SNAPSHOT_CHUNK,
    PATH_PREFIX_SNAPSHOT_METADATA, PATH_PREFIX_STAKING_ACCOUNT, TYPE_ACCOUNT,
    TYPE_APP_STATE_SNAPSHOT, TYPE_APP_STATE_TIP, TYPE_CADO_MAP, TYPE_CHUNK_REFERENCE,
    TYPE_EPOCH_RECORD, TYPE_NAMESPACE, TYPE_SNAPSHOT_CHUNK, TYPE_SNAPSHOT_METADATA,
    TYPE_STAKING_ACCOUNT, TYPE_STORAGE_STAKING_ACCOUNT,
};
use rocksdb::{ColumnFamilyDescriptor, Direction, IteratorMode, Options, DB};
use std::path::Path;
use std::time::Instant;
use tracing::error;

const ROCKSDB_TOTAL_SST_FILES_SIZE: &str = "rocksdb.total-sst-files-size";
const INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY: &[u8] =
    b"_meta:indexed_transactions_primary_total";
const INDEXED_EVENTS_PRIMARY_PREFIX: &[u8] = b"tx_id:";
const MAX_KEYS_PER_PREFIX: usize = 5_000_000;
const PATH_PREFIX_STORAGE_STAKING_ACCOUNT: &str = "/@eld/storage_staking_account/";

const PINBOARD_BLOB_CF_NAMES: &[&str] = &["pinboard_temp_blobs"];

const ELD_CF_NAMES: &[&str] = &[
    "cado",
    "path_index",
    "cado_map",
    "indexed_transactions",
    "indexed_events",
    "missing_content",
    "pinboard_meta",
    "pinboard_temp_blobs",
    "pinboard_refcounts",
    "pinboard_idx_wallet",
    "pinboard_idx_tag",
    "pinboard_idx_expiry",
    "pinboard_idx_commit",
    "pinboard_gc_meta",
    "pinboard_gc_deleted_log",
    "verified_proof_submissions",
    "default",
];

const CADO_TYPE_PREFIXES: &[(&str, &str)] = &[
    (TYPE_ACCOUNT, PATH_PREFIX_ACCOUNT),
    (TYPE_STAKING_ACCOUNT, PATH_PREFIX_STAKING_ACCOUNT),
    (
        TYPE_STORAGE_STAKING_ACCOUNT,
        PATH_PREFIX_STORAGE_STAKING_ACCOUNT,
    ),
    (TYPE_CADO_MAP, PATH_PREFIX_CADO_MAP),
    (TYPE_APP_STATE_TIP, PATH_PREFIX_APP_STATE_TIP),
    (TYPE_EPOCH_RECORD, PATH_PREFIX_EPOCH_RECORD),
    (TYPE_NAMESPACE, PATH_PREFIX_NAMESPACE_REGISTRY),
    (TYPE_SNAPSHOT_METADATA, PATH_PREFIX_SNAPSHOT_METADATA),
    (TYPE_SNAPSHOT_CHUNK, PATH_PREFIX_SNAPSHOT_CHUNK),
    (TYPE_CHUNK_REFERENCE, PATH_PREFIX_CHUNK_REFERENCE),
    (TYPE_APP_STATE_SNAPSHOT, PATH_PREFIX_APP_STATE_SNAPSHOT),
    ("pinboard", PATH_PREFIX_PINBOARD),
];

fn eld_column_family_descriptors() -> Vec<ColumnFamilyDescriptor> {
    ELD_CF_NAMES
        .iter()
        .filter(|&&name| name != "default")
        .map(|&name| ColumnFamilyDescriptor::new(name, Options::default()))
        .collect()
}

enum InventoryError {
    Failed(String),
    TimedOut,
}

/// Collect inventory from an Eld RocksDB directory.
/// Returns `None` when the path is missing or the database cannot be opened.
/// A timeout or a later read error keeps the counts and sizes already finished.
pub fn collect_inventory(rocksdb_path: &Path) -> Option<RocksDbInventory> {
    collect_inventory_until(rocksdb_path, Instant::now() + SCAN_BUDGET)
}

pub(crate) fn collect_inventory_until(
    rocksdb_path: &Path,
    deadline: Instant,
) -> Option<RocksDbInventory> {
    match collect_inventory_inner(rocksdb_path, deadline) {
        Ok(inv) => Some(inv),
        Err(InventoryError::TimedOut) => {
            error!(
                path = %rocksdb_path.display(),
                "RocksDB inventory exceeded 30s before any counts were read"
            );
            None
        }
        Err(InventoryError::Failed(err)) => {
            error!(
                path = %rocksdb_path.display(),
                error = %err,
                "RocksDB inventory: read-only open failed"
            );
            None
        }
    }
}

fn collect_inventory_inner(
    rocksdb_path: &Path,
    deadline: Instant,
) -> Result<RocksDbInventory, InventoryError> {
    if Instant::now() >= deadline {
        return Err(InventoryError::TimedOut);
    }
    if !rocksdb_path.exists() {
        return Err(InventoryError::Failed(format!(
            "path does not exist: {}",
            rocksdb_path.display()
        )));
    }

    let opts = Options::default();
    let cfs = eld_column_family_descriptors();
    let db = DB::open_cf_descriptors_read_only(&opts, rocksdb_path, cfs, false)
        .map_err(|e| InventoryError::Failed(e.to_string()))?;

    if Instant::now() >= deadline {
        return Err(InventoryError::TimedOut);
    }

    let cado_type_counts = count_cado_types(&db, deadline);
    let indexed_transactions_count = count_indexed_transactions(&db, deadline);
    let indexed_events_count = count_indexed_events(&db, deadline);

    let mut cf_sst_bytes = Vec::new();
    let mut pinboard_blob = 0u64;
    let mut other = 0u64;
    let mut saw_any_sst = false;

    for &name in ELD_CF_NAMES {
        if Instant::now() >= deadline {
            error!(
                path = %rocksdb_path.display(),
                "RocksDB sst size scan exceeded 30s; keeping sizes already read"
            );
            break;
        }
        let Some(cf) = db.cf_handle(name) else {
            continue;
        };
        let sz = match db.property_int_value_cf(cf, ROCKSDB_TOTAL_SST_FILES_SIZE) {
            Ok(value) => value.unwrap_or(0),
            Err(err) => {
                error!(
                    column_family = name,
                    error = %err,
                    "RocksDB sst size read failed; keeping sizes already read"
                );
                continue;
            }
        };
        saw_any_sst = true;
        cf_sst_bytes.push(CfSstBytes {
            column_family: name.to_string(),
            sst_bytes: sz,
        });
        if PINBOARD_BLOB_CF_NAMES.contains(&name) {
            pinboard_blob = pinboard_blob.saturating_add(sz);
        } else {
            other = other.saturating_add(sz);
        }
    }

    Ok(RocksDbInventory {
        cado_type_counts,
        indexed_transactions_count,
        indexed_events_count,
        cf_sst_bytes,
        pinboard_blob_cf_sst_bytes: saw_any_sst.then_some(pinboard_blob),
        other_cf_sst_bytes: saw_any_sst.then_some(other),
    })
}

fn count_cado_types(db: &DB, deadline: Instant) -> Vec<CadoTypeCount> {
    let Some(cf) = db.cf_handle("path_index") else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(CADO_TYPE_PREFIXES.len());
    for &(cado_type, prefix) in CADO_TYPE_PREFIXES {
        if Instant::now() >= deadline {
            error!(
                cado_type,
                "cado type count exceeded 30s; keeping counts already finished"
            );
            break;
        }
        match count_keys_with_prefix(db, cf, prefix.as_bytes(), deadline) {
            Ok(Some(count)) => out.push(CadoTypeCount {
                cado_type: cado_type.to_string(),
                count,
            }),
            Ok(None) => {
                error!(
                    cado_type,
                    "cado type count exceeded 30s; keeping counts already finished"
                );
                break;
            }
            Err(err) => {
                error!(
                    cado_type,
                    error = %err_label(&err),
                    "cado type count failed; keeping counts already finished"
                );
            }
        }
    }
    out
}

/// `Ok(None)` means the 30s budget ran out before this prefix finished.
fn count_keys_with_prefix(
    db: &DB,
    cf: &rocksdb::ColumnFamily,
    prefix: &[u8],
    deadline: Instant,
) -> Result<Option<u64>, InventoryError> {
    if Instant::now() >= deadline {
        return Ok(None);
    }
    let iter = db.iterator_cf(cf, IteratorMode::From(prefix, Direction::Forward));
    let mut count = 0u64;
    let mut visited = 0usize;
    for item in iter {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let (key, _) = item.map_err(|e| InventoryError::Failed(e.to_string()))?;
        if !key.starts_with(prefix) {
            break;
        }
        count = count.saturating_add(1);
        visited = visited.saturating_add(1);
        if visited >= MAX_KEYS_PER_PREFIX {
            break;
        }
    }
    Ok(Some(count))
}

fn count_indexed_transactions(db: &DB, deadline: Instant) -> Option<u64> {
    if Instant::now() >= deadline {
        error!("indexed transaction count exceeded 30s; leaving that count unset");
        return None;
    }
    let cf = db.cf_handle("indexed_transactions")?;
    if let Ok(Some(bytes)) = db.get_cf(cf, INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY) {
        if bytes.len() == 8 {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes);
            return Some(u64::from_le_bytes(arr));
        }
    }
    if Instant::now() >= deadline {
        error!("indexed transaction count exceeded 30s; leaving that count unset");
        return None;
    }
    // Fallback: count primary `0x…` keys without `:`.
    let mut iter = db.iterator_cf(cf, IteratorMode::From(b"0x", Direction::Forward));
    let mut count = 0u64;
    let mut visited = 0usize;
    loop {
        if Instant::now() >= deadline {
            error!("indexed transaction count exceeded 30s; leaving that count unset");
            return None;
        }
        match iter.next() {
            Some(Ok((key, _))) => {
                if !key.starts_with(b"0x") {
                    break;
                }
                if !key.contains(&b':') {
                    count = count.saturating_add(1);
                }
                visited = visited.saturating_add(1);
                if visited >= MAX_KEYS_PER_PREFIX {
                    break;
                }
            }
            Some(Err(err)) => {
                error!(
                    error = %err,
                    "indexed transaction count failed; leaving that count unset"
                );
                return None;
            }
            None => break,
        }
    }
    Some(count)
}

fn count_indexed_events(db: &DB, deadline: Instant) -> Option<u64> {
    let cf = db.cf_handle("indexed_events")?;
    match count_keys_with_prefix(db, cf, INDEXED_EVENTS_PRIMARY_PREFIX, deadline) {
        Ok(Some(count)) => Some(count),
        Ok(None) => {
            error!("indexed event count exceeded 30s; leaving that count unset");
            None
        }
        Err(err) => {
            error!(
                error = %err_label(&err),
                "indexed event count failed; leaving that count unset"
            );
            None
        }
    }
}

fn err_label(err: &InventoryError) -> String {
    match err {
        InventoryError::Failed(message) => message.clone(),
        InventoryError::TimedOut => "scan exceeded 30s".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::{Options, DB};
    use tempfile::tempdir;

    fn open_test_db(path: &Path) -> DB {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let cfs = eld_column_family_descriptors();
        DB::open_cf_descriptors(&opts, path, cfs).expect("open test db")
    }

    #[test]
    fn inventory_counts_path_index_and_indexer_meta() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rocksdb");
        {
            let db = open_test_db(&path);
            let path_index = db.cf_handle("path_index").unwrap();
            db.put_cf(path_index, b"/@eld/account/0xaaa", b"k1")
                .unwrap();
            db.put_cf(path_index, b"/@eld/account/0xbbb", b"k2")
                .unwrap();
            db.put_cf(path_index, b"/@eld/epoch_record/1", b"k3")
                .unwrap();

            let txs = db.cf_handle("indexed_transactions").unwrap();
            db.put_cf(
                txs,
                INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY,
                7u64.to_le_bytes(),
            )
            .unwrap();

            let events = db.cf_handle("indexed_events").unwrap();
            db.put_cf(events, b"tx_id:0x1:event_index:0", b"{}")
                .unwrap();
            db.put_cf(events, b"tx_id:0x1:event_index:1", b"{}")
                .unwrap();
            db.put_cf(events, b"type:Transfer:0x1", b"1").unwrap();
        }

        let inv = collect_inventory(&path).expect("inventory");
        let account = inv
            .cado_type_counts
            .iter()
            .find(|c| c.cado_type == TYPE_ACCOUNT)
            .unwrap();
        assert_eq!(account.count, 2);
        let epoch = inv
            .cado_type_counts
            .iter()
            .find(|c| c.cado_type == TYPE_EPOCH_RECORD)
            .unwrap();
        assert_eq!(epoch.count, 1);
        assert_eq!(inv.indexed_transactions_count, Some(7));
        assert_eq!(inv.indexed_events_count, Some(2));
        assert!(!inv.cf_sst_bytes.is_empty());
    }

    #[test]
    fn missing_path_returns_none() {
        let dir = tempdir().unwrap();
        assert!(collect_inventory(&dir.path().join("nope")).is_none());
    }

    #[test]
    fn spent_budget_skips_inventory_without_opening() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rocksdb");
        let _db = open_test_db(&path);
        let deadline = Instant::now() - std::time::Duration::from_secs(1);
        assert!(collect_inventory_until(&path, deadline).is_none());
    }
}
