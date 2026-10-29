//! Tendermint summary–driven indexer prune loop.
//!
//! Separate from the sync thread. Every five minutes, asks the Tendermint node
//! for the current [`SummaryHeader`](eld_client::api::abci::SummaryHeader) and
//! deletes indexed data for heights after genesis up through `end_height`.
//! Failures stay on this thread and never enter ABCI.

use crate::indexer::TransactionIndexer;
use eld_client::api::abci::AbciHttpApi;
use eld_common::error::EldError;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const PRUNE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const RETRY_SLEEP: Duration = Duration::from_secs(1);

/// Grep-friendly prefix for indexer prune thread logs.
const IX: &str = "INDEXER-PRUNE-LOG:";

/// Genesis block height. Never pruned; passed as the exclusive lower bound.
const GENESIS_HEIGHT: u64 = 1;

/// Spawn a dedicated thread that prunes the indexer from Tendermint summaries.
///
/// Independent of consensus and of the indexer sync thread.
pub fn spawn_indexer_prune(indexer: Arc<TransactionIndexer>, tendermint_rpc_url: String) {
    std::thread::spawn(move || {
        info!(
            tendermint_rpc = %tendermint_rpc_url,
            "{} transaction indexer prune thread starting",
            IX
        );
        loop {
            let indexer = Arc::clone(&indexer);
            let url = tendermint_rpc_url.clone();
            let result = catch_unwind(AssertUnwindSafe(|| {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        error!(
                            error = %e,
                            "{} failed to build indexer prune Tokio runtime; will retry after sleep",
                            IX
                        );
                        std::thread::sleep(RETRY_SLEEP);
                        return;
                    }
                };
                rt.block_on(run_indexer_prune_loop(indexer, url));
            }));
            match result {
                Ok(()) => {
                    warn!("{} indexer prune loop exited; restarting after sleep", IX);
                }
                Err(_) => {
                    error!(
                        "{} indexer prune panicked; restarting after sleep (ABCI unaffected)",
                        IX
                    );
                }
            }
            std::thread::sleep(RETRY_SLEEP);
        }
    });
}

async fn run_indexer_prune_loop(indexer: Arc<TransactionIndexer>, tendermint_rpc_url: String) {
    let api = match AbciHttpApi::new(tendermint_rpc_url) {
        Ok(api) => api,
        Err(e) => {
            error!(
                error = %e,
                "{} failed to create Tendermint RPC client; indexer prune will not start",
                IX
            );
            return;
        }
    };

    loop {
        match prune_once_from_summary(&indexer, &api).await {
            Ok(PruneTick::SkippedNoSummary) => {
                debug!("{} no summary yet; skipping prune this tick", IX);
            }
            Ok(PruneTick::SkippedLowEndHeight { end_height }) => {
                debug!(
                    end_height,
                    "{} summary end_height too low to prune; skipping", IX
                );
            }
            Ok(PruneTick::Pruned { end_height }) => {
                info!(
                    end_height,
                    start_exclusive = GENESIS_HEIGHT,
                    end_exclusive = end_height.saturating_add(1),
                    "{} indexer pruned from summary",
                    IX
                );
            }
            Err(e) => {
                error!(error = %e, "{} indexer prune_between failed", IX);
            }
        }
        tokio::time::sleep(PRUNE_INTERVAL).await;
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PruneTick {
    SkippedNoSummary,
    SkippedLowEndHeight { end_height: u64 },
    Pruned { end_height: u64 },
}

async fn prune_once_from_summary(
    indexer: &TransactionIndexer,
    api: &AbciHttpApi,
) -> Result<PruneTick, EldError> {
    let header = match api.get_summary().await {
        Ok(header) => header,
        Err(EldError::NotFoundError { .. }) => return Ok(PruneTick::SkippedNoSummary),
        Err(e) => return Err(e),
    };

    let end_height: u64 = header
        .end_height
        .parse()
        .map_err(|e| EldError::NetworkError {
            operation: "summary_end_height".to_string(),
            details: format!(
                "Failed to parse summary end_height {:?}: {e}",
                header.end_height
            ),
        })?;

    if end_height <= GENESIS_HEIGHT {
        return Ok(PruneTick::SkippedLowEndHeight { end_height });
    }

    let end_exclusive = end_height.saturating_add(1);
    indexer.prune_between(GENESIS_HEIGHT, end_exclusive)?;
    Ok(PruneTick::Pruned { end_height })
}
