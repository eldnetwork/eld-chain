//! Read-only inventory of an Eld ABCI RocksDB directory.

use crate::model::{CadoTypeCount, CfSstBytes, RocksDbInventory};
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
use tracing::warn;

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

/// Collect inventory from an Eld RocksDB directory. Returns `None` on open/scan failure.
pub fn collect_inventory(rocksdb_path: &Path) -> Option<RocksDbInventory> {
    match collect_inventory_inner(rocksdb_path) {
        Ok(inv) => Some(inv),
        Err(err) => {
            warn!(
                path = %rocksdb_path.display(),
                error = %err,
                "RocksDB inventory: read-only open or scan failed"
            );
            None
        }
    }
}

fn collect_inventory_inner(rocksdb_path: &Path) -> Result<RocksDbInventory, String> {
    if !rocksdb_path.exists() {
        return Err(format!("path does not exist: {}", rocksdb_path.display()));
    }

    let opts = Options::default();
    let cfs = eld_column_family_descriptors();
    let db = DB::open_cf_descriptors_read_only(&opts, rocksdb_path, cfs, false)
        .map_err(|e| e.to_string())?;

    let cado_type_counts = count_cado_types(&db)?;
    let indexed_transactions_count = count_indexed_transactions(&db);
    let indexed_events_count = count_indexed_events(&db);

    let mut cf_sst_bytes = Vec::new();
    let mut pinboard_blob = 0u64;
    let mut other = 0u64;
    let mut saw_any_sst = false;

    for &name in ELD_CF_NAMES {
        let Some(cf) = db.cf_handle(name) else {
            continue;
        };
        let sz = db
            .property_int_value_cf(cf, ROCKSDB_TOTAL_SST_FILES_SIZE)
            .map_err(|e| e.to_string())?
            .unwrap_or(0);
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

fn count_cado_types(db: &DB) -> Result<Vec<CadoTypeCount>, String> {
    let Some(cf) = db.cf_handle("path_index") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(CADO_TYPE_PREFIXES.len());
    for &(cado_type, prefix) in CADO_TYPE_PREFIXES {
        let count = count_keys_with_prefix(db, cf, prefix.as_bytes())?;
        out.push(CadoTypeCount {
            cado_type: cado_type.to_string(),
            count,
        });
    }
    Ok(out)
}

fn count_keys_with_prefix(
    db: &DB,
    cf: &rocksdb::ColumnFamily,
    prefix: &[u8],
) -> Result<u64, String> {
    let iter = db.iterator_cf(cf, IteratorMode::From(prefix, Direction::Forward));
    let mut count = 0u64;
    let mut visited = 0usize;
    for item in iter {
        let (key, _) = item.map_err(|e| e.to_string())?;
        if !key.starts_with(prefix) {
            break;
        }
        count = count.saturating_add(1);
        visited = visited.saturating_add(1);
        if visited >= MAX_KEYS_PER_PREFIX {
            break;
        }
    }
    Ok(count)
}

fn count_indexed_transactions(db: &DB) -> Option<u64> {
    let cf = db.cf_handle("indexed_transactions")?;
    if let Ok(Some(bytes)) = db.get_cf(cf, INDEXED_TRANSACTIONS_PRIMARY_TOTAL_COUNT_KEY) {
        if bytes.len() == 8 {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes);
            return Some(u64::from_le_bytes(arr));
        }
    }
    // Fallback: count primary `0x…` keys without `:`.
    let mut iter = db.iterator_cf(cf, IteratorMode::From(b"0x", Direction::Forward));
    let mut count = 0u64;
    let mut visited = 0usize;
    while let Some(Ok((key, _))) = iter.next() {
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
    Some(count)
}

fn count_indexed_events(db: &DB) -> Option<u64> {
    let cf = db.cf_handle("indexed_events")?;
    count_keys_with_prefix(db, cf, INDEXED_EVENTS_PRIMARY_PREFIX).ok()
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
}
