//! Filesystem size helpers for analytics collection.

use crate::model::TopFile;
use crate::scan::SCAN_BUDGET;
use std::path::{Path, PathBuf};
use std::time::Instant;

const TOP_FILES_MAX_DEPTH: usize = 12;
const TOP_FILES_MAX_VISITED: usize = 50_000;

enum Walk<T> {
    Done(T),
    TimedOut,
}

/// Recursive sum of file sizes under `path`. Missing path → `0`.
/// A walk that is still going at 30 seconds stops and returns `0` so the caller can move on.
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

/// Bytes reported by `statvfs` for the filesystem containing `path`.
#[derive(Debug, Clone, Copy)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[cfg(unix)]
fn block_count_to_u64(count: impl Into<u64>) -> u64 {
    count.into()
}

#[cfg(unix)]
pub fn statvfs_for_path(path: &Path) -> Option<DiskUsage> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `statvfs` writes to `stat` when it returns 0.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    let frsize = stat.f_frsize;
    let total_bytes = block_count_to_u64(stat.f_blocks).saturating_mul(frsize);
    let available_bytes = block_count_to_u64(stat.f_bavail).saturating_mul(frsize);
    Some(DiskUsage {
        total_bytes,
        available_bytes,
    })
}

#[cfg(not(unix))]
pub fn statvfs_for_path(_path: &Path) -> Option<DiskUsage> {
    None
}

/// Largest `n` files under `root`, with depth / visit / time caps.
pub fn top_n_files(root: &Path, n: usize) -> Vec<TopFile> {
    if n == 0 || !root.exists() {
        return Vec::new();
    }
    let started = Instant::now();
    let mut heap: Vec<(u64, PathBuf)> = Vec::new();
    let mut visited = 0usize;
    if walk_top_files(root, 0, n, &mut heap, &mut visited, started) == WalkStop::TimedOut {
        tracing::error!(
            path = %root.display(),
            "largest-file scan exceeded 30s; keeping files found so far"
        );
    }
    heap.sort_by(|a, b| b.0.cmp(&a.0));
    heap.into_iter()
        .take(n)
        .map(|(size_bytes, path)| TopFile {
            relative_path: path_relative_display(root, &path),
            size_bytes,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WalkStop {
    Done,
    TimedOut,
    VisitCap,
}

fn walk_top_files(
    path: &Path,
    depth: usize,
    n: usize,
    heap: &mut Vec<(u64, PathBuf)>,
    visited: &mut usize,
    started: Instant,
) -> WalkStop {
    if started.elapsed() >= SCAN_BUDGET {
        return WalkStop::TimedOut;
    }
    if *visited >= TOP_FILES_MAX_VISITED {
        return WalkStop::VisitCap;
    }
    if depth > TOP_FILES_MAX_DEPTH {
        return WalkStop::Done;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return WalkStop::Done,
    };
    for entry in entries.flatten() {
        if started.elapsed() >= SCAN_BUDGET {
            return WalkStop::TimedOut;
        }
        if *visited >= TOP_FILES_MAX_VISITED {
            return WalkStop::VisitCap;
        }
        *visited = visited.saturating_add(1);
        let p = entry.path();
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.is_dir() {
            match walk_top_files(&p, depth + 1, n, heap, visited, started) {
                WalkStop::Done => {}
                stop => return stop,
            }
        } else {
            insert_top(heap, n, md.len(), p);
        }
    }
    WalkStop::Done
}

fn insert_top(heap: &mut Vec<(u64, PathBuf)>, n: usize, size: u64, path: PathBuf) {
    if heap.len() < n {
        heap.push((size, path));
        return;
    }
    if let Some((min_i, _)) = heap.iter().enumerate().min_by_key(|(_, (s, _))| *s) {
        if size > heap[min_i].0 {
            heap[min_i] = (size, path);
        }
    }
}

fn path_relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
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

    #[test]
    fn top_n_files_stops_when_the_budget_is_already_spent() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("big"), b"abcdefghij").unwrap();
        let started = Instant::now() - SCAN_BUDGET - std::time::Duration::from_secs(1);
        let mut heap = Vec::new();
        let mut visited = 0usize;
        assert_eq!(
            walk_top_files(dir.path(), 0, 2, &mut heap, &mut visited, started),
            WalkStop::TimedOut
        );
        assert!(heap.is_empty());
    }

    #[test]
    fn top_n_files_orders_by_size() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("small"), b"a").unwrap();
        fs::write(dir.path().join("big"), b"abcdefghij").unwrap();
        fs::write(dir.path().join("mid"), b"abcd").unwrap();
        let tops = top_n_files(dir.path(), 2);
        assert_eq!(tops.len(), 2);
        assert_eq!(tops[0].relative_path, "big");
        assert_eq!(tops[0].size_bytes, 10);
        assert_eq!(tops[1].relative_path, "mid");
    }
}
