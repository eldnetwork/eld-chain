//! Filesystem size helpers for analytics collection.

use crate::model::TopFile;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TOP_FILES_MAX_DEPTH: usize = 12;
const TOP_FILES_MAX_VISITED: usize = 50_000;
const TOP_FILES_TIME_BUDGET: Duration = Duration::from_secs(30);

/// Recursive sum of file sizes under `path`. Missing path → `0`.
pub fn directory_tree_size_bytes(path: &Path) -> u64 {
    directory_tree_size_bytes_inner(path).unwrap_or_default()
}

fn directory_tree_size_bytes_inner(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    if !path.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        let md = entry.metadata()?;
        if md.is_dir() {
            total = total.saturating_add(directory_tree_size_bytes_inner(&p)?);
        } else {
            total = total.saturating_add(md.len());
        }
    }
    Ok(total)
}

/// Size of a file or directory tree. Missing path → `0`.
pub fn file_or_dir_size(path: &Path) -> u64 {
    directory_tree_size_bytes(path)
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
    let _ = walk_top_files(root, 0, n, &mut heap, &mut visited, started);
    heap.sort_by(|a, b| b.0.cmp(&a.0));
    heap.into_iter()
        .take(n)
        .map(|(size_bytes, path)| TopFile {
            relative_path: path_relative_display(root, &path),
            size_bytes,
        })
        .collect()
}

fn walk_top_files(
    path: &Path,
    depth: usize,
    n: usize,
    heap: &mut Vec<(u64, PathBuf)>,
    visited: &mut usize,
    started: Instant,
) -> bool {
    if started.elapsed() > TOP_FILES_TIME_BUDGET || *visited >= TOP_FILES_MAX_VISITED {
        return false;
    }
    if depth > TOP_FILES_MAX_DEPTH {
        return true;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return true,
    };
    for entry in entries.flatten() {
        if started.elapsed() > TOP_FILES_TIME_BUDGET || *visited >= TOP_FILES_MAX_VISITED {
            return false;
        }
        *visited = visited.saturating_add(1);
        let p = entry.path();
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.is_dir() {
            if !walk_top_files(&p, depth + 1, n, heap, visited, started) {
                return false;
            }
        } else {
            insert_top(heap, n, md.len(), p);
        }
    }
    true
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
        assert_eq!(file_or_dir_size(&dir.path().join("missing")), 0);
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
