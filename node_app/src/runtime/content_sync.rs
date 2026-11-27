use std::sync::{Arc, Mutex};

use tracing::{error, info};

use crate::capacity::capacity_manager::CapacityManager;
use crate::capacity::missing_content_tracker::MissingContentTracker;
use crate::content::sync::{PeriodicSyncConfig, PeriodicSyncService};

/// Load the missing-content tracker and start the periodic sync check.
pub fn init_content_sync(capacity_manager: Arc<CapacityManager>) {
    let capacity_dir = capacity_manager.config().capacity_dir.as_path();
    let mut tracker = MissingContentTracker::new(capacity_dir);
    tracker.load().unwrap_or_else(|e| {
        error!(error = %e, "Failed to load missing content tracker");
    });
    let tracker = Arc::new(Mutex::new(tracker));

    let periodic_sync_service =
        PeriodicSyncService::new(capacity_manager, tracker, PeriodicSyncConfig::default());
    tokio::spawn(async move {
        if let Err(e) = periodic_sync_service.start().await {
            error!(error = %e, "Periodic sync service failed");
        }
    });
    info!("Periodic sync service started");
}
