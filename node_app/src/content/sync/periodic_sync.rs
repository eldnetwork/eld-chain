use crate::capacity::capacity_manager::CapacityManager;
use crate::capacity::missing_content_tracker::MissingContentTracker;
use eld_common::error::EldError;
use eld_common::missing_content::MissingContentRecord;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;
use tracing::{error, info};

/// Maximum missing-content records checked in one periodic sync pass.
const MAX_MISSING_CONTENT_PER_PASS: usize = 32;

/// Oldest attempts first, so a backlog larger than `limit` rotates across passes.
fn select_missing_for_pass(
    mut records: Vec<MissingContentRecord>,
    limit: usize,
) -> Vec<MissingContentRecord> {
    records.sort_by_key(|record| (record.last_attempt.unwrap_or(0), record.discovered_at));
    records.truncate(limit);
    records
}

/// Configuration for periodic sync service
#[derive(Debug, Clone)]
pub struct PeriodicSyncConfig {
    /// Interval for periodic sync check (seconds)
    pub check_interval_secs: u64, // Default: 300 (5 minutes)
}

impl Default for PeriodicSyncConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 300,
        }
    }
}

/// Periodic sync service that checks for missing content and clears entries now in slots.
pub struct PeriodicSyncService {
    missing_content_tracker: Arc<Mutex<MissingContentTracker>>,
    capacity_manager: Arc<CapacityManager>,
    config: PeriodicSyncConfig,
}

impl PeriodicSyncService {
    /// Create a new periodic sync service.
    pub fn new(
        capacity_manager: Arc<CapacityManager>,
        missing_content_tracker: Arc<Mutex<MissingContentTracker>>,
        config: PeriodicSyncConfig,
    ) -> Self {
        Self {
            missing_content_tracker,
            capacity_manager,
            config,
        }
    }

    /// Start the periodic sync service
    pub async fn start(&self) -> Result<(), EldError> {
        let interval = Duration::from_secs(self.config.check_interval_secs);
        let mut interval_timer = tokio::time::interval(interval);
        // Skip the first tick to avoid immediate execution
        interval_timer.tick().await;

        info!(
            "Periodic sync service started (interval: {} seconds)",
            self.config.check_interval_secs
        );

        loop {
            interval_timer.tick().await;
            info!("Running periodic content sync check");

            if let Err(e) = self.check_missing_content().await {
                error!(error = %e, "Error in periodic sync check");
            }
        }
    }

    /// Drop tracker rows whose bytes are now in slots, and record another look at the rest.
    async fn check_missing_content(&self) -> Result<(), EldError> {
        // Get all missing content
        let missing = {
            let tracker = self.missing_content_tracker.lock().unwrap();
            tracker.get_all_missing_content()?
        };

        if missing.is_empty() {
            info!("No missing content found, skipping sync");
            return Ok(());
        }

        let total = missing.len();
        let batch = select_missing_for_pass(missing, MAX_MISSING_CONTENT_PER_PASS);
        info!(
            count = total,
            checking = batch.len(),
            "Found missing content entries"
        );

        for record in batch {
            // Double-check: is content still missing? (check in slots)
            match self
                .capacity_manager
                .get_content_from_slots(&record.content_id.hex_with_prefix())
                .await
            {
                Ok(Some(_)) => {
                    // Content exists! Remove from missing list (cleanup)
                    info!(
                        manifest_id = %record.manifest_id,
                        "Content now exists in slots, removing from missing list"
                    );
                    let mut tracker = self.missing_content_tracker.lock().unwrap();
                    tracker.mark_content_synced(&record.manifest_id.hex_with_prefix())?;
                }
                Ok(None) => {
                    info!(
                        manifest_id = %record.manifest_id,
                        content_id = %record.content_id,
                        attempt_count = record.attempt_count,
                        "Content still missing from capacity slots"
                    );

                    // Update attempt tracking
                    let mut updated_record = record;
                    updated_record.attempt_count += 1;
                    updated_record.last_attempt = Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_err(|e| EldError::StorageError {
                                operation: "get_system_time".to_string(),
                                details: format!("Failed to get system time: {e}"),
                            })?
                            .as_secs(),
                    );

                    // Store updated record
                    let mut tracker = self.missing_content_tracker.lock().unwrap();
                    tracker.update_missing_content_record(&updated_record)?;
                }
                Err(e) => {
                    error!(
                        manifest_id = %record.manifest_id,
                        content_id = %record.content_id,
                        error = %e,
                        "Failed to read content from slots"
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{select_missing_for_pass, MAX_MISSING_CONTENT_PER_PASS};
    use eld_common::content_id::ContentId;
    use eld_common::manifest_id::ManifestId;
    use eld_common::missing_content::MissingContentRecord;

    fn record(n: u8, last_attempt: Option<u64>, discovered_at: u64) -> MissingContentRecord {
        let mut bytes = [0u8; 32];
        bytes[0] = n;
        MissingContentRecord {
            manifest_id: ManifestId::new(bytes),
            content_id: ContentId::new(bytes),
            block_height: 0,
            discovered_at,
            attempt_count: 0,
            last_attempt,
        }
    }

    #[test]
    fn selects_oldest_up_to_limit() {
        let mut records: Vec<_> = (0u8..40)
            .map(|i| record(i, Some(u64::from(i) + 1), u64::from(i)))
            .collect();
        records.reverse();

        let selected = select_missing_for_pass(records, MAX_MISSING_CONTENT_PER_PASS);
        let attempts: Vec<_> = selected.iter().map(|r| r.last_attempt).collect();
        let expected: Vec<_> = (0u8..32).map(|i| Some(u64::from(i) + 1)).collect();
        assert_eq!(attempts, expected);
    }

    #[test]
    fn next_pass_continues_with_records_not_yet_selected() {
        let mut records: Vec<_> = (0u8..40)
            .map(|i| record(i, Some(u64::from(i) + 1), u64::from(i)))
            .collect();
        let first = select_missing_for_pass(records.clone(), MAX_MISSING_CONTENT_PER_PASS);
        let selected_ids: Vec<_> = first.iter().map(|r| r.manifest_id).collect::<Vec<_>>();

        for record in &mut records {
            if selected_ids.contains(&record.manifest_id) {
                record.last_attempt = Some(1_000 + record.last_attempt.unwrap_or(0));
            }
        }

        let second = select_missing_for_pass(records, MAX_MISSING_CONTENT_PER_PASS);
        let leading: Vec<_> = second.iter().take(8).map(|r| r.last_attempt).collect();
        let expected: Vec<_> = (32u8..40).map(|i| Some(u64::from(i) + 1)).collect();
        assert_eq!(leading, expected);
        assert_eq!(second.len(), MAX_MISSING_CONTENT_PER_PASS);
    }

    #[test]
    fn empty_and_short_inputs_return_everything() {
        assert!(select_missing_for_pass(Vec::new(), MAX_MISSING_CONTENT_PER_PASS).is_empty());

        let records = vec![record(1, Some(5), 2), record(2, None, 9)];
        let selected = select_missing_for_pass(records, MAX_MISSING_CONTENT_PER_PASS);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].last_attempt, None);
        assert_eq!(selected[1].last_attempt, Some(5));
    }
}
