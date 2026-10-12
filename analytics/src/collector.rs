use crate::config::Config;
use crate::disk::{directory_tree_size_bytes, file_or_dir_size, statvfs_for_path, top_n_files};
use crate::model::{AbciDiskSnapshot, CollectionRun, TendermintDiskSnapshot};
use crate::rocksdb_inventory::collect_inventory;
use chrono::Utc;
use std::path::{Path, PathBuf};

const TOP_FILES_N: usize = 5;

pub fn collect_once(config: &Config, block_height: Option<u64>) -> CollectionRun {
    let mut tendermint_nodes = Vec::with_capacity(config.node_ids.len());
    let mut abci_apps = Vec::with_capacity(config.node_ids.len());

    for id in &config.node_ids {
        tendermint_nodes.push(collect_tendermint(config, id));
        abci_apps.push(collect_abci(config, id));
    }

    CollectionRun {
        collected_at: Utc::now(),
        block_height,
        tendermint_nodes,
        abci_apps,
    }
}

fn tendermint_home(config: &Config, id: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", config.tendermint_path_prefix, id))
}

fn tendermint_data_dir(config: &Config, home: &Path) -> PathBuf {
    if config.tendermint_data_subdir.is_empty() {
        home.to_path_buf()
    } else {
        home.join(&config.tendermint_data_subdir)
    }
}

fn collect_tendermint(config: &Config, id: &str) -> TendermintDiskSnapshot {
    let home = tendermint_home(config, id);
    let data = tendermint_data_dir(config, &home);
    let volume = statvfs_for_path(&home);

    TendermintDiskSnapshot {
        node_id: id.to_string(),
        home_dir_bytes: directory_tree_size_bytes(&home),
        data_dir_bytes: directory_tree_size_bytes(&data),
        blockstore_db_bytes: file_or_dir_size(&data.join("blockstore.db")),
        state_db_bytes: file_or_dir_size(&data.join("state.db")),
        cs_wal_bytes: file_or_dir_size(&data.join("cs.wal")),
        evidence_db_bytes: file_or_dir_size(&data.join("evidence.db")),
        tx_index_db_bytes: file_or_dir_size(&data.join("tx_index.db")),
        volume_total_bytes: volume.map(|v| v.total_bytes),
        volume_available_bytes: volume.map(|v| v.available_bytes),
        top_files: top_n_files(&data, TOP_FILES_N),
    }
}

fn collect_abci(config: &Config, id: &str) -> AbciDiskSnapshot {
    let root = PathBuf::from(format!("{}{}", config.abci_path_prefix, id));
    let rocks = root.join("rocksdb");
    let capacity = root.join("capacity");
    let volume = statvfs_for_path(&root);

    AbciDiskSnapshot {
        node_id: id.to_string(),
        data_dir_bytes: directory_tree_size_bytes(&root),
        rocksdb_bytes: directory_tree_size_bytes(&rocks),
        capacity_bytes: directory_tree_size_bytes(&capacity),
        volume_total_bytes: volume.map(|v| v.total_bytes),
        volume_available_bytes: volume.map(|v| v.available_bytes),
        top_files: top_n_files(&root, TOP_FILES_N),
        rocksdb_inventory: collect_inventory(&rocks),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn collect_once_reads_fixture_trees() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        let tm = base.join("tendermint-1").join("data");
        fs::create_dir_all(tm.join("blockstore.db")).unwrap();
        fs::write(tm.join("blockstore.db/CURRENT"), b"MANIFEST-000001").unwrap();
        fs::create_dir_all(tm.join("cs.wal")).unwrap();
        fs::write(tm.join("cs.wal/wal"), b"wal-bytes").unwrap();

        let eld = base.join("eld-1");
        fs::create_dir_all(eld.join("rocksdb")).unwrap();
        fs::write(eld.join("rocksdb/IDENTITY"), b"x").unwrap();
        fs::create_dir_all(eld.join("capacity")).unwrap();
        fs::write(eld.join("capacity/slot0"), b"slot").unwrap();

        let config = Config {
            db_path: base.join("analytics.db"),
            listen: "127.0.0.1:0".into(),
            first_collection_delay_secs: 180,
            interval_secs: 3600,
            node_ids: vec!["1".into()],
            tendermint_path_prefix: format!("{}/tendermint-", base.display()),
            tendermint_data_subdir: "data".into(),
            abci_path_prefix: format!("{}/eld-", base.display()),
            tendermint_rpc_url: "http://127.0.0.1:26657".into(),
        };

        let run = collect_once(&config, Some(42));
        assert_eq!(run.block_height, Some(42));
        assert_eq!(run.tendermint_nodes.len(), 1);
        assert_eq!(run.abci_apps.len(), 1);
        assert!(run.tendermint_nodes[0].blockstore_db_bytes > 0);
        assert!(run.tendermint_nodes[0].cs_wal_bytes > 0);
        assert!(run.abci_apps[0].rocksdb_bytes > 0);
        assert!(run.abci_apps[0].capacity_bytes > 0);
        // Empty / non-Eld RocksDB layout → inventory None
        assert!(run.abci_apps[0].rocksdb_inventory.is_none());
    }
}
