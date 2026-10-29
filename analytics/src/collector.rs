use crate::config::Config;
use crate::disk::directory_tree_size_bytes;
use crate::model::{AbciDiskSnapshot, CollectionRun, TendermintDiskSnapshot};
use crate::scan::scan;
use chrono::Utc;
use std::path::{Path, PathBuf};

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

    TendermintDiskSnapshot {
        node_id: id.to_string(),
        data_dir_bytes: sized("tendermint data", &data),
        tx_index_db_bytes: sized("tendermint tx index", &data.join("tx_index.db")),
        evidence_db_bytes: sized("tendermint evidence", &data.join("evidence.db")),
    }
}

fn sized(measurement: &'static str, path: &Path) -> u64 {
    let path_buf = path.to_path_buf();
    scan(measurement, path, 0, move || {
        directory_tree_size_bytes(&path_buf)
    })
}

fn collect_abci(config: &Config, id: &str) -> AbciDiskSnapshot {
    let root = PathBuf::from(format!("{}{}", config.abci_path_prefix, id));
    let rocks = root.join("rocksdb");
    let capacity = root.join("capacity");

    AbciDiskSnapshot {
        node_id: id.to_string(),
        data_dir_bytes: sized("abci data", &root),
        rocksdb_bytes: sized("abci rocksdb", &rocks),
        capacity_bytes: sized("abci capacity", &capacity),
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
        fs::create_dir_all(tm.join("tx_index.db")).unwrap();
        fs::write(tm.join("tx_index.db/CURRENT"), b"MANIFEST-000001").unwrap();
        fs::create_dir_all(tm.join("evidence.db")).unwrap();
        fs::write(tm.join("evidence.db/CURRENT"), b"ev").unwrap();

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
        assert!(run.tendermint_nodes[0].data_dir_bytes > 0);
        assert!(run.tendermint_nodes[0].tx_index_db_bytes > 0);
        assert!(run.tendermint_nodes[0].evidence_db_bytes > 0);
        assert!(run.abci_apps[0].rocksdb_bytes > 0);
        assert!(run.abci_apps[0].capacity_bytes > 0);
    }
}
