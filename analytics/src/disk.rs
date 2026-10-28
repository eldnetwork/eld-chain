//! Folder size helpers for analytics collection.

use crate::scan::SCAN_BUDGET;
use std::path::Path;
use std::time::Instant;

enum Walk<T> {
    Done(T),
    TimedOut,
}

/// Sum of file sizes under `path`. A missing path is `0`.
/// A walk still going at 30 seconds stops and returns `0`.
pub fn directory_tree_size_bytes(path: &Path) -> u64 {
    let started = Instant::now();
    match directory_tree_size_bytes_inner(path, started) {
        Ok(Walk::Done(total)) => total,
        Ok(Walk::TimedOut) => {
            tracing::error!(
                path = %path.display(),
                "directory size scan exceeded 30s; skipping this measurement"
            );
            0
        }
        Err(_) => 0,
    }
}

fn directory_tree_size_bytes_inner(path: &Path, started: Instant) -> std::io::Result<Walk<u64>> {
    if started.elapsed() >= SCAN_BUDGET {
        return Ok(Walk::TimedOut);
    }
    if path.is_file() {
        return Ok(Walk::Done(path.metadata()?.len()));
    }
    if !path.exists() {
        return Ok(Walk::Done(0));
    }
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        if started.elapsed() >= SCAN_BUDGET {
            return Ok(Walk::TimedOut);
        }
        let entry = entry?;
        let p = entry.path();
        let md = entry.metadata()?;
        if md.is_dir() {
            match directory_tree_size_bytes_inner(&p, started)? {
                Walk::Done(bytes) => total = total.saturating_add(bytes),
                Walk::TimedOut => return Ok(Walk::TimedOut),
            }
        } else {
            total = total.saturating_add(md.len());
        }
    }
    Ok(Walk::Done(total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn directory_tree_size_sums_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.txt"), b"world!").unwrap();
        assert_eq!(directory_tree_size_bytes(dir.path()), 11);
        assert_eq!(directory_tree_size_bytes(&dir.path().join("missing")), 0);
    }

    #[test]
    fn directory_tree_size_stops_when_the_budget_is_already_spent() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        let started = Instant::now() - SCAN_BUDGET - std::time::Duration::from_secs(1);
        assert!(matches!(
            directory_tree_size_bytes_inner(dir.path(), started).unwrap(),
            Walk::TimedOut
        ));
    }
}
