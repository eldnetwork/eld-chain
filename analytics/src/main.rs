mod api;
mod collector;
mod config;
mod disk;
mod memory;
mod model;
mod rocksdb_inventory;
mod rpc;
mod scan;
mod store;

use crate::api::{router, AppState};
use crate::collector::collect_once;
use crate::config::Config;
use crate::memory::collect_memory;
use crate::rpc::fetch_latest_block_height;
use crate::store::AnalyticsStore;
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::parse();
    let store = Arc::new(AnalyticsStore::open(&config.db_path)?);
    let state = Arc::new(AppState {
        store: store.clone(),
    });

    let collector_config = config.clone();
    let collector_store = store.clone();
    tokio::spawn(async move {
        let delay_secs = collector_config.first_collection_delay_secs;
        if delay_secs > 0 {
            info!(
                delay_secs,
                "waiting before first collection (lets the cluster settle)"
            );
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        let mut interval =
            tokio::time::interval(Duration::from_secs(collector_config.interval_secs.max(1)));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            let config = collector_config.clone();
            let store = collector_store.clone();
            tokio::spawn(async move {
                let block_height = match fetch_latest_block_height(&config.tendermint_rpc_url).await
                {
                    Ok(height) => Some(height),
                    Err(e) => {
                        error!(
                            rpc_url = %config.tendermint_rpc_url,
                            "failed to fetch block height: {e:#}"
                        );
                        None
                    }
                };
                let cfg = config.clone();
                let joined =
                    tokio::task::spawn_blocking(move || collect_once(&cfg, block_height)).await;
                match joined {
                    Ok(mut run) => {
                        let memory = collect_memory(&config).await;
                        run.vm_memory = memory.vm;
                        run.tendermint_memory = memory.tendermint;
                        run.abci_memory = memory.abci;
                        if let Err(e) = store.put_run(&run) {
                            error!("failed to store collection run: {e:#}");
                        } else {
                            info!(
                                collected_at = %run.collected_at,
                                block_height = ?run.block_height,
                                tm_nodes = run.tendermint_nodes.len(),
                                abci_apps = run.abci_apps.len(),
                                vm_mem_available_bytes = run.vm_memory.map(|m| m.mem_available_bytes),
                                "stored collection run"
                            );
                        }
                    }
                    Err(e) => error!("collection worker failed: {e}"),
                }
            });
        }
    });

    let app = router(state);
    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    info!("listening on {}", config.listen);
    axum::serve(listener, app).await?;
    Ok(())
}
