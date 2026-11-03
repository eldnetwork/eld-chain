//! Deletes pinboard metadata and indexes for pins EndBlock has dropped.
//!
//! EndBlock sends the dropped set on a channel. This worker does the RocksDB
//! deletes. A failure stays on this task and never enters ABCI.

use crate::app_state::LivePin;
use crate::capacity::capacity_manager::CapacityManager;
use crate::storage::rocksdb::RocksDBStorage;
use eld_common::capacity::slot_allocator::CONTENT_ID_NOT_IN_SLOT_MAP;
use eld_common::error::EldError;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

pub fn start_pinboard_prune_worker(
    storage: Arc<RocksDBStorage>,
    capacity_manager: Arc<CapacityManager>,
    mut dropped_rx: mpsc::UnboundedReceiver<Vec<LivePin>>,
) {
    tokio::spawn(async move {
        info!("Starting pinboard metadata prune worker");
        while let Some(dropped) = dropped_rx.recv().await {
            prune_dropped_pins(storage.as_ref(), capacity_manager.as_ref(), dropped).await;
        }
    });
}

async fn prune_dropped_pins(
    storage: &RocksDBStorage,
    capacity_manager: &CapacityManager,
    dropped: Vec<LivePin>,
) {
    for pin in dropped {
        let content_key = match storage.delete_expired_pinboard_message(&pin.message_id) {
            Ok(content_key) => content_key,
            Err(e) => {
                error!(
                    error = %e,
                    message_id = %pin.message_id,
                    "Pinboard metadata prune failed deleting rows"
                );
                continue;
            }
        };
        let Some(content_key) = content_key else {
            continue;
        };
        match capacity_manager
            .delete_content_from_slots(&content_key)
            .await
        {
            Ok(()) => {}
            Err(EldError::NotFoundError { resource_type, .. })
                if resource_type == CONTENT_ID_NOT_IN_SLOT_MAP =>
            {
                // Content is already gone from local slots.
            }
            Err(e) => {
                error!(
                    content_key = %content_key,
                    message_id = %pin.message_id,
                    error = %e,
                    "Pinboard metadata prune failed deleting content from capacity slots"
                );
            }
        }
    }
}
