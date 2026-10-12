use crate::model::CollectionRun;
use anyhow::{Context, Result};
use rocksdb::{IteratorMode, Options, DB};
use std::path::Path;

pub struct AnalyticsStore {
    db: DB,
}

impl AnalyticsStore {
    pub fn open(path: &Path) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create analytics db parent {}", parent.display()))?;
        }
        let db = DB::open(&opts, path)
            .with_context(|| format!("open analytics rocksdb at {}", path.display()))?;
        Ok(Self { db })
    }

    pub fn put_run(&self, run: &CollectionRun) -> Result<()> {
        let key = run.collected_at.timestamp_millis().to_be_bytes();
        let value = bincode::serialize(run).context("serialize CollectionRun")?;
        self.db.put(key, value).context("put CollectionRun")?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<CollectionRun>> {
        // Skip legacy/unreadable rows (e.g. pre-block_height bincode).
        for item in self.db.iterator(IteratorMode::End) {
            let (_key, value) = item?;
            match bincode::deserialize::<CollectionRun>(&value) {
                Ok(run) => return Ok(Some(run)),
                Err(e) => {
                    tracing::warn!("skipping unreadable CollectionRun in latest: {e}");
                }
            }
        }
        Ok(None)
    }

    pub fn history(&self, limit: usize) -> Result<Vec<CollectionRun>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(limit.min(64));
        for item in self.db.iterator(IteratorMode::End) {
            if out.len() >= limit {
                break;
            }
            let (_key, value) = item?;
            match bincode::deserialize::<CollectionRun>(&value) {
                Ok(run) => out.push(run),
                Err(e) => {
                    tracing::warn!("skipping unreadable CollectionRun in history: {e}");
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AbciDiskSnapshot, TendermintDiskSnapshot};
    use chrono::Utc;
    use tempfile::tempdir;

    fn sample_run() -> CollectionRun {
        CollectionRun {
            collected_at: Utc::now(),
            block_height: Some(100),
            tendermint_nodes: vec![TendermintDiskSnapshot {
                node_id: "1".into(),
                home_dir_bytes: 10,
                data_dir_bytes: 9,
                blockstore_db_bytes: 5,
                state_db_bytes: 1,
                cs_wal_bytes: 1,
                evidence_db_bytes: 0,
                tx_index_db_bytes: 2,
                volume_total_bytes: Some(100),
                volume_available_bytes: Some(50),
                top_files: vec![],
            }],
            abci_apps: vec![AbciDiskSnapshot {
                node_id: "1".into(),
                data_dir_bytes: 20,
                rocksdb_bytes: 15,
                capacity_bytes: 5,
                volume_total_bytes: Some(100),
                volume_available_bytes: Some(40),
                top_files: vec![],
                rocksdb_inventory: None,
            }],
        }
    }

    #[test]
    fn put_latest_history_round_trip() {
        let dir = tempdir().unwrap();
        let store = AnalyticsStore::open(&dir.path().join("db")).unwrap();
        let run = sample_run();
        store.put_run(&run).unwrap();
        let latest = store.latest().unwrap().expect("latest");
        assert_eq!(latest.block_height, Some(100));
        assert_eq!(latest.tendermint_nodes[0].node_id, "1");
        assert_eq!(latest.abci_apps[0].rocksdb_bytes, 15);
        let hist = store.history(10).unwrap();
        assert_eq!(hist.len(), 1);
    }
}
