//! Per-measurement deadline. A scan that does not finish is skipped so the rest of the run continues.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) const SCAN_BUDGET: Duration = Duration::from_secs(30);

/// Extra wait so a scan that stops itself at [`SCAN_BUDGET`] can deliver that result.
/// A scan still blocked in a syscall is abandoned after this grace.
const SCAN_JOIN_GRACE: Duration = Duration::from_millis(500);

pub(crate) fn scan<T>(
    measurement: &'static str,
    path: &Path,
    fallback: T,
    work: impl FnOnce() -> T + Send + 'static,
) -> T
where
    T: Send + 'static,
{
    scan_within(measurement, path, SCAN_BUDGET, fallback, work)
}

pub(crate) fn scan_within<T>(
    measurement: &'static str,
    path: &Path,
    budget: Duration,
    fallback: T,
    work: impl FnOnce() -> T + Send + 'static,
) -> T
where
    T: Send + 'static,
{
    let path_log = path.display().to_string();
    let (tx, rx) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("eld-scan".to_string())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(work));
            let _ = tx.send(outcome);
        });
    if let Err(err) = spawned {
        tracing::warn!(
            measurement,
            path = %path_log,
            error = %err,
            "could not start scan; continuing with the rest of the collection"
        );
        return fallback;
    }

    match rx.recv_timeout(budget.saturating_add(SCAN_JOIN_GRACE)) {
        Ok(Ok(value)) => value,
        Ok(Err(_)) => {
            tracing::warn!(
                measurement,
                path = %path_log,
                "scan panicked; continuing with the rest of the collection"
            );
            fallback
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            tracing::error!(
                measurement,
                path = %path_log,
                budget_secs = budget.as_secs(),
                "scan exceeded 30s; skipping this measurement and continuing"
            );
            fallback
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            tracing::warn!(
                measurement,
                path = %path_log,
                "scan ended without a result; continuing with the rest of the collection"
            );
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn finished_scan_returns_its_value() {
        let path = PathBuf::from("/unused");
        let value = scan_within("fast", &path, Duration::from_secs(2), 0, || 7u64);
        assert_eq!(value, 7);
    }

    #[test]
    fn slow_scan_returns_fallback_and_does_not_block() {
        let path = PathBuf::from("/unused");
        let value = scan_within("slow", &path, Duration::from_millis(40), 0, || {
            thread::sleep(Duration::from_millis(800));
            1u64
        });
        assert_eq!(value, 0);
    }

    #[test]
    fn panicked_scan_returns_fallback() {
        let path = PathBuf::from("/unused");
        let value = scan_within("panic", &path, Duration::from_secs(2), 5u64, || -> u64 {
            panic!("scan failed");
        });
        assert_eq!(value, 5);
    }
}
