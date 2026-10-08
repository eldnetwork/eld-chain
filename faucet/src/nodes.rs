use chrono::{DateTime, Utc};
use eld_client::api::abci::AbciHttpApi;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const SAMPLE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct NodeSample {
    pub url: String,
    pub host: String,
    pub port: String,
    pub height: u64,
    pub block_time: DateTime<Utc>,
    pub chain_id: String,
}

#[derive(Debug, Clone)]
pub struct Quorum {
    /// URL to broadcast to.
    pub target: String,
    /// Host for [`crate::config::FaucetConfig::to_client_config`].
    pub target_host: String,
    /// Port for [`crate::config::FaucetConfig::to_client_config`].
    pub target_port: String,
    /// Max height in the tip group.
    pub height: u64,
    pub synced: Vec<String>,
}

#[derive(Debug, Clone)]
struct TipSnapshot {
    height: u64,
    newest_block_time: DateTime<Utc>,
}

/// One poll target: host, port, and resolved RPC base URL.
#[derive(Debug, Clone)]
struct PoolEntry {
    host: String,
    port: String,
    url: String,
}

pub struct NodePool {
    entries: Vec<PoolEntry>,
    chain_id: String,
    min_synced: usize,
    max_block_age: Duration,
    poll_interval: Duration,
    latest: Mutex<Option<Quorum>>,
    seen: Mutex<Option<TipSnapshot>>,
    excluded: Mutex<HashMap<String, Instant>>,
}

impl NodePool {
    pub fn new(
        hosts: Vec<String>,
        ports: Vec<String>,
        urls: Vec<String>,
        chain_id: String,
        min_synced: usize,
        max_block_age: Duration,
        poll_interval: Duration,
    ) -> Self {
        assert_eq!(hosts.len(), ports.len());
        assert_eq!(hosts.len(), urls.len());
        Self {
            entries: hosts
                .into_iter()
                .zip(ports)
                .zip(urls)
                .map(|((host, port), url)| PoolEntry { host, port, url })
                .collect(),
            chain_id,
            min_synced,
            max_block_age,
            poll_interval,
            latest: Mutex::new(None),
            seen: Mutex::new(None),
            excluded: Mutex::new(HashMap::new()),
        }
    }

    pub fn has_quorum(&self) -> bool {
        self.latest.lock().expect("quorum mutex").is_some()
    }

    /// `(host, port, url)` of the current quorum broadcast target.
    pub fn target(&self) -> Option<(String, String, String)> {
        self.latest.lock().expect("quorum mutex").as_ref().map(|q| {
            (
                q.target_host.clone(),
                q.target_port.clone(),
                q.target.clone(),
            )
        })
    }

    pub fn exclude(&self, url: &str) {
        self.excluded
            .lock()
            .expect("excluded mutex")
            .insert(url.to_string(), Instant::now());
    }

    pub async fn poll(&self) {
        let samples = self.sample_all().await;
        let now = Utc::now();
        let fresh: Vec<NodeSample> = samples
            .into_iter()
            .filter(|s| {
                if s.chain_id != self.chain_id {
                    warn!(url = %s.url, expected = %self.chain_id, actual = %s.chain_id, "reject node: chain id mismatch");
                    return false;
                }
                if s.height == 0 {
                    warn!(url = %s.url, "reject node: height 0");
                    return false;
                }
                let age = now.signed_duration_since(s.block_time);
                if age.num_seconds() > self.max_block_age.as_secs() as i64 {
                    warn!(url = %s.url, age_secs = age.num_seconds(), "reject node: block too old");
                    return false;
                }
                true
            })
            .collect();

        let excluded = self.excluded.lock().expect("excluded mutex");
        let exclude_ttl = self.poll_interval;
        let is_excluded = |url: &str| {
            excluded
                .get(url)
                .is_some_and(|at| at.elapsed() < exclude_ttl)
        };

        let Some(built) = build_quorum(&fresh, self.min_synced, &is_excluded) else {
            drop(excluded);
            warn!("no tip quorum (need {} synced at tip)", self.min_synced);
            *self.latest.lock().expect("quorum mutex") = None;
            return;
        };
        drop(excluded);

        let tip = TipSnapshot {
            height: built.quorum.height,
            newest_block_time: built.newest_block_time,
        };
        let progressed = {
            let mut seen = self.seen.lock().expect("seen mutex");
            let prog = match seen.as_ref() {
                None => false,
                Some(prev) => tip_has_progress(
                    prev.height,
                    prev.newest_block_time,
                    tip.height,
                    tip.newest_block_time,
                ),
            };
            *seen = Some(tip);
            prog
        };

        let mut latest = self.latest.lock().expect("quorum mutex");
        if progressed || latest.is_some() {
            info!(
                target = %built.quorum.target,
                height = built.quorum.height,
                synced = built.quorum.synced.len(),
                "faucet node quorum"
            );
            *latest = Some(built.quorum);
        } else {
            info!(
                height = built.quorum.height,
                "tip group ok; waiting for height or block-time progress"
            );
            *latest = None;
        }
    }

    async fn sample_all(&self) -> Vec<NodeSample> {
        let mut handles = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let host = entry.host.clone();
            let port = entry.port.clone();
            let url = entry.url.clone();
            handles.push(tokio::spawn(
                async move { sample_one(host, port, url).await },
            ));
        }
        let mut out = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Some(sample)) => out.push(sample),
                Ok(None) => {}
                Err(e) => warn!("status sample task failed: {e}"),
            }
        }
        out
    }
}

async fn sample_one(host: String, port: String, url: String) -> Option<NodeSample> {
    let host_label = host.clone();
    let port_label = port.clone();
    let result = tokio::time::timeout(SAMPLE_TIMEOUT, async {
        let api = AbciHttpApi::new(url.clone()).ok()?;
        let tip = api.status_tip().await.ok()?;
        let block_time = DateTime::from_timestamp(tip.block_time_secs, 0)?;
        Some(NodeSample {
            url,
            host,
            port,
            height: tip.height,
            block_time,
            chain_id: tip.chain_id,
        })
    })
    .await;

    match result {
        Ok(sample) => sample,
        Err(_) => {
            warn!(host = %host_label, port = %port_label, "status sample timed out");
            None
        }
    }
}

pub(crate) struct BuiltQuorum {
    pub(crate) quorum: Quorum,
    pub(crate) newest_block_time: DateTime<Utc>,
}

/// Pure tip-group selection (no RPC). `is_excluded` skips URLs when picking `target`.
pub(crate) fn build_quorum(
    samples: &[NodeSample],
    min_synced: usize,
    is_excluded: &dyn Fn(&str) -> bool,
) -> Option<BuiltQuorum> {
    if samples.is_empty() {
        return None;
    }
    let max_height = samples.iter().map(|s| s.height).max()?;
    let tip_floor = max_height.saturating_sub(1);
    let tip: Vec<&NodeSample> = samples.iter().filter(|s| s.height >= tip_floor).collect();
    if tip.len() < min_synced {
        return None;
    }

    let newest_block_time = tip.iter().map(|s| s.block_time).max()?;
    let synced: Vec<String> = tip.iter().map(|s| s.url.clone()).collect();

    let mut candidates: Vec<&NodeSample> = tip
        .iter()
        .copied()
        .filter(|s| !is_excluded(&s.url))
        .collect();
    if candidates.is_empty() {
        candidates = tip;
    }
    let chosen = candidates
        .into_iter()
        .max_by_key(|s| s.height)
        .expect("non-empty tip");

    Some(BuiltQuorum {
        quorum: Quorum {
            target: chosen.url.clone(),
            target_host: chosen.host.clone(),
            target_port: chosen.port.clone(),
            height: max_height,
            synced,
        },
        newest_block_time,
    })
}

/// Whether a new tip observation counts as progress vs a previous snapshot.
pub(crate) fn tip_has_progress(
    prev_height: u64,
    prev_time: DateTime<Utc>,
    height: u64,
    time: DateTime<Utc>,
) -> bool {
    height > prev_height || time > prev_time
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn sample(url: &str, host: &str, port: &str, height: u64, age_secs: i64) -> NodeSample {
        NodeSample {
            url: url.to_string(),
            host: host.to_string(),
            port: port.to_string(),
            height,
            block_time: Utc::now() - TimeDelta::seconds(age_secs),
            chain_id: "eld-testnet-tempelhof".to_string(),
        }
    }

    #[test]
    fn tip_group_selects_height_64_over_lagging_node() {
        let samples = vec![
            sample("http://n1:26657/", "n1", "26657", 64, 1),
            sample("http://n2:26667/", "n2", "26667", 64, 1),
            sample("http://n3:26677/", "n3", "26677", 63, 1),
            sample("http://n4:26687/", "n4", "26687", 1, 1),
        ];
        let built = build_quorum(&samples, 3, &|_| false).expect("quorum");
        assert_eq!(built.quorum.height, 64);
        assert!(
            built.quorum.target == "http://n1:26657/" || built.quorum.target == "http://n2:26667/"
        );
        assert!(!built.quorum.synced.iter().any(|u| u == "http://n4:26687/"));
        assert_eq!(built.quorum.synced.len(), 3);
    }

    #[test]
    fn stale_block_times_yield_no_quorum_when_filtered() {
        // Caller filters by max_block_age before build_quorum; empty → None.
        let samples: Vec<NodeSample> = vec![];
        assert!(build_quorum(&samples, 3, &|_| false).is_none());
    }

    #[test]
    fn three_stale_samples_still_form_tip_if_passed_in() {
        // Document: three nodes at 64 with old times produce no quorum — that is
        // enforced by poll() age filter, not build_quorum. Age-filter unit:
        let now = Utc::now();
        let stale = now - TimeDelta::seconds(20);
        let samples = [
            NodeSample {
                url: "http://n1:26657/".into(),
                host: "n1".into(),
                port: "26657".into(),
                height: 64,
                block_time: stale,
                chain_id: "eld-testnet-tempelhof".into(),
            },
            NodeSample {
                url: "http://n2:26667/".into(),
                host: "n2".into(),
                port: "26667".into(),
                height: 64,
                block_time: stale,
                chain_id: "eld-testnet-tempelhof".into(),
            },
            NodeSample {
                url: "http://n3:26677/".into(),
                host: "n3".into(),
                port: "26677".into(),
                height: 64,
                block_time: stale,
                chain_id: "eld-testnet-tempelhof".into(),
            },
        ];
        let max_age = Duration::from_secs(15);
        let fresh: Vec<_> = samples
            .into_iter()
            .filter(|s| {
                now.signed_duration_since(s.block_time).num_seconds() <= max_age.as_secs() as i64
            })
            .collect();
        assert!(build_quorum(&fresh, 3, &|_| false).is_none());
    }

    #[test]
    fn two_at_tip_is_below_min_synced() {
        let samples = vec![
            sample("http://n1:26657/", "n1", "26657", 64, 1),
            sample("http://n2:26667/", "n2", "26667", 64, 1),
        ];
        assert!(build_quorum(&samples, 3, &|_| false).is_none());
    }

    #[test]
    fn same_height_and_time_is_not_progress() {
        let t = Utc::now();
        assert!(!tip_has_progress(64, t, 64, t));
        assert!(tip_has_progress(64, t, 65, t));
        assert!(tip_has_progress(64, t, 64, t + TimeDelta::seconds(1)));
    }

    #[test]
    fn excluded_target_skipped_when_alternative_exists() {
        let samples = vec![
            sample("http://n1:26657/", "n1", "26657", 64, 1),
            sample("http://n2:26667/", "n2", "26667", 64, 1),
            sample("http://n3:26677/", "n3", "26677", 63, 1),
        ];
        let built = build_quorum(&samples, 3, &|url| url == "http://n1:26657/").expect("quorum");
        assert_eq!(built.quorum.target, "http://n2:26667/");
        assert_eq!(built.quorum.target_port, "26667");
    }
}
