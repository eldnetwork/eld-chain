//! After the first consensus commit: provider lookup and on-chain register.

use std::sync::Arc;

use eld_client::api::abci::AbciHttpApi;
use eld_common::error::EldError;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

use crate::capacity::capacity_manager::CapacityManager;
use crate::errors::handle_fatal_eld_error;

/// Wait for first commit, then register capacity if allocated.
pub async fn after_first_commit(
    ready_rx: oneshot::Receiver<()>,
    capacity_manager: Arc<CapacityManager>,
) {
    info!("Waiting for node to be ready (block height > 0)...");
    match ready_rx.await {
        Ok(_) => {
            info!("Node is READY: First consensus commit completed, block height > 0");
            check_existing_registration(&capacity_manager).await;
            submit_registration_if_allocated(&capacity_manager).await;
        }
        Err(_) => {
            warn!("Ready channel was closed before signaling - node may not be ready");
        }
    }
}

async fn check_existing_registration(capacity_manager: &CapacityManager) {
    let provider_id = capacity_manager.config().provider_id;
    let provider_id_str = provider_id.to_string();
    let tendermint_rpc_url = capacity_manager.config().tendermint_rpc_url.clone();

    match AbciHttpApi::new(tendermint_rpc_url) {
        Ok(abci_api) => match abci_api
            .is_capacity_provider_registered(&provider_id_str)
            .await
        {
            Ok(true) => {
                info!(
                    provider_id = %provider_id,
                    "Current node is registered as a capacity provider"
                );
            }
            Ok(false) => {
                info!(
                    provider_id = %provider_id,
                    "Current node is not yet registered as a capacity provider"
                );
            }
            Err(e) => {
                warn!(
                    provider_id = %provider_id,
                    error = %e,
                    "Failed to check if node is registered as capacity provider"
                );
            }
        },
        Err(e) => {
            warn!(
                provider_id = %provider_id,
                error = %e,
                "Failed to create ABCI HTTP client to check capacity provider registration"
            );
        }
    }
}

async fn submit_registration_if_allocated(capacity_manager: &CapacityManager) {
    let slot_allocator = capacity_manager.slot_allocator();
    let slot_allocator_guard = slot_allocator.lock().await;
    let capacity_allocated = slot_allocator_guard.slot_map_exists();
    drop(slot_allocator_guard);

    if !capacity_allocated {
        info!("Capacity not allocated, skipping registration");
        return;
    }

    match capacity_manager.get_registration_info().await {
        Ok((capacity_bytes, seed, merkle_root)) => {
            info!("Submitting capacity registration transaction...");
            if let Err(e) = capacity_manager
                .register_capacity_onchain(capacity_bytes, seed, merkle_root)
                .await
            {
                let wallet_missing = matches!(
                    &e,
                    EldError::StorageError { operation, details }
                        if operation == "register_capacity_onchain"
                            && details.contains("Failed to get wallet")
                );
                if wallet_missing {
                    handle_fatal_eld_error(e);
                }
                error!(
                    error = %e,
                    "Failed to submit capacity registration transaction"
                );
            } else {
                info!("Capacity registration transaction submitted successfully");
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                "Failed to get registration info - capacity may not be allocated"
            );
        }
    }
}
