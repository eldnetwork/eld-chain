//! VM memory from `/proc/meminfo`, and per-container cgroup memory from the Docker API.

use crate::config::Config;
use crate::model::{AbciMemorySnapshot, ProcessMemory, TendermintMemorySnapshot, VmMemorySnapshot};
use crate::rpc::fetch_mempool_totals;
use anyhow::Context;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const MEMINFO_PATH: &str = "/proc/meminfo";
/// Docker reports a limit near 2^63 when the container has no memory cap.
const UNLIMITED_MEMORY_BYTES: u64 = 1u64 << 50;

pub struct MemorySample {
    pub vm: Option<VmMemorySnapshot>,
    pub tendermint: Vec<TendermintMemorySnapshot>,
    pub abci: Vec<AbciMemorySnapshot>,
}

pub async fn collect_memory(config: &Config) -> MemorySample {
    let vm = match read_proc_meminfo() {
        Ok(snapshot) => Some(snapshot),
        Err(e) => {
            tracing::error!("failed to read vm memory: {e:#}");
            None
        }
    };

    let cfg = config.clone();
    let containers = tokio::task::spawn_blocking(move || container_memory(&cfg))
        .await
        .unwrap_or_else(|e| {
            tracing::error!("container memory task failed: {e}");
            let n = config.node_ids.len();
            (
                vec![ProcessMemory::default(); n],
                vec![ProcessMemory::default(); n],
            )
        });

    let mempools = mempool_by_node(config).await;
    let mut tendermint = Vec::with_capacity(config.node_ids.len());
    let mut abci = Vec::with_capacity(config.node_ids.len());
    for (i, id) in config.node_ids.iter().enumerate() {
        let (mempool_txs, mempool_bytes) = mempools[i];
        tendermint.push(TendermintMemorySnapshot {
            node_id: id.clone(),
            memory: containers.0[i],
            mempool_txs,
            mempool_bytes,
        });
        abci.push(AbciMemorySnapshot {
            node_id: id.clone(),
            memory: containers.1[i],
        });
    }

    MemorySample {
        vm,
        tendermint,
        abci,
    }
}

fn read_proc_meminfo() -> anyhow::Result<VmMemorySnapshot> {
    let text =
        std::fs::read_to_string(MEMINFO_PATH).with_context(|| format!("read {MEMINFO_PATH}"))?;
    parse_meminfo(&text)
}

pub(crate) fn parse_meminfo(text: &str) -> anyhow::Result<VmMemorySnapshot> {
    let mut total = None;
    let mut available = None;
    let mut swap_total = None;
    let mut swap_free = None;
    let mut anon = None;
    let mut cached = None;

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let key = key.trim_end_matches(':');
        let Some(raw) = parts.next() else {
            continue;
        };
        let Ok(value) = raw.parse::<u64>() else {
            continue;
        };
        let unit = parts.next().unwrap_or("kB");
        let bytes = if unit == "kB" { value * 1024 } else { value };
        match key {
            "MemTotal" => total = Some(bytes),
            "MemAvailable" => available = Some(bytes),
            "SwapTotal" => swap_total = Some(bytes),
            "SwapFree" => swap_free = Some(bytes),
            "AnonPages" => anon = Some(bytes),
            "Cached" => cached = Some(bytes),
            _ => {}
        }
    }

    Ok(VmMemorySnapshot {
        mem_total_bytes: total.context("MemTotal")?,
        mem_available_bytes: available.context("MemAvailable")?,
        swap_total_bytes: swap_total.context("SwapTotal")?,
        swap_free_bytes: swap_free.context("SwapFree")?,
        anon_pages_bytes: anon.context("AnonPages")?,
        cached_bytes: cached.context("Cached")?,
    })
}

fn container_memory(config: &Config) -> (Vec<ProcessMemory>, Vec<ProcessMemory>) {
    let n = config.node_ids.len();
    if !docker_socket_ready(&config.docker_socket) {
        return (
            vec![ProcessMemory::default(); n],
            vec![ProcessMemory::default(); n],
        );
    }

    let mut tendermint = Vec::with_capacity(n);
    let mut abci = Vec::with_capacity(n);
    for id in &config.node_ids {
        tendermint.push(memory_for(
            &config.docker_socket,
            &config.tendermint_container_name(id),
        ));
        abci.push(memory_for(
            &config.docker_socket,
            &config.abci_container_name(id),
        ));
    }
    (tendermint, abci)
}

fn docker_socket_ready(socket: &Path) -> bool {
    match UnixStream::connect(socket) {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(
                socket = %socket.display(),
                "docker socket unavailable; container memory left empty: {e}"
            );
            false
        }
    }
}

fn memory_for(socket: &Path, container: &str) -> ProcessMemory {
    match docker_memory(socket, container) {
        Ok(memory) => memory,
        Err(e) => {
            tracing::error!(container, "container memory: {e:#}");
            ProcessMemory::default()
        }
    }
}

fn docker_memory(socket: &Path, container: &str) -> anyhow::Result<ProcessMemory> {
    let mut stream =
        UnixStream::connect(socket).with_context(|| format!("connect {}", socket.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .context("set docker socket read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("set docker socket write timeout")?;

    let path = format!("/v1.41/containers/{container}/stats?stream=false");
    let request = format!("GET {path} HTTP/1.0\r\nHost: docker\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .context("write docker stats request")?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .context("read docker stats response")?;
    let value = http_json(&raw).with_context(|| format!("docker stats for {container}"))?;
    parse_process_memory(&value)
}

pub(crate) fn parse_process_memory(body: &serde_json::Value) -> anyhow::Result<ProcessMemory> {
    let memory_stats = body
        .get("memory_stats")
        .ok_or_else(|| anyhow::anyhow!("docker stats missing memory_stats"))?;
    let stats = memory_stats.get("stats");
    let usage_bytes = json_u64(memory_stats.get("usage"));
    let anon_bytes = stat(stats, "anon")
        .or_else(|| stat(stats, "rss"))
        .or(sum_stats(stats, "active_anon", "inactive_anon"));
    let file_bytes = stat(stats, "file")
        .or_else(|| stat(stats, "cache"))
        .or(sum_stats(stats, "active_file", "inactive_file"));
    let swap_bytes = stat(stats, "swap");
    let limit_bytes = json_u64(memory_stats.get("limit")).and_then(usable_limit);
    let peak_bytes = json_u64(memory_stats.get("max_usage"))
        .and_then(usable_peak)
        .or_else(|| stat(stats, "peak").and_then(usable_peak));

    Ok(ProcessMemory {
        usage_bytes,
        anon_bytes,
        file_bytes,
        swap_bytes,
        limit_bytes,
        peak_bytes,
    })
}

fn stat(stats: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    json_u64(stats?.get(key))
}

fn sum_stats(stats: Option<&serde_json::Value>, a: &str, b: &str) -> Option<u64> {
    match (stat(stats, a), stat(stats, b)) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0) + right.unwrap_or(0)),
    }
}

fn usable_limit(limit: u64) -> Option<u64> {
    if limit == 0 || limit >= UNLIMITED_MEMORY_BYTES {
        None
    } else {
        Some(limit)
    }
}

fn usable_peak(peak: u64) -> Option<u64> {
    if peak == 0 || peak >= UNLIMITED_MEMORY_BYTES {
        None
    } else {
        Some(peak)
    }
}

fn json_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    let n = value.as_f64()?;
    if n >= 0.0 {
        Some(n as u64)
    } else {
        None
    }
}

async fn mempool_by_node(config: &Config) -> Vec<(Option<u64>, Option<u64>)> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("mempool client: {e}");
            return vec![(None, None); config.node_ids.len()];
        }
    };

    let mut handles = Vec::with_capacity(config.node_ids.len());
    for id in &config.node_ids {
        let client = client.clone();
        let url = config.tendermint_rpc_url_for(id);
        let node_id = id.clone();
        handles.push(tokio::spawn(async move {
            match fetch_mempool_totals(&client, &url).await {
                Ok((txs, bytes)) => (Some(txs), Some(bytes)),
                Err(e) => {
                    tracing::error!(node_id = %node_id, url = %url, "mempool: {e:#}");
                    (None, None)
                }
            }
        }));
    }

    let mut out = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(pair) => out.push(pair),
            Err(e) => {
                tracing::error!("mempool task failed: {e}");
                out.push((None, None));
            }
        }
    }
    out
}

fn http_json(raw: &[u8]) -> anyhow::Result<serde_json::Value> {
    let (header, body) = split_http(raw)?;
    let header = std::str::from_utf8(header).context("docker stats header is not utf-8")?;
    let code = status_code(header)?;
    if !(200..300).contains(&code) {
        anyhow::bail!("docker stats HTTP {code}");
    }
    let payload = decode_http_body(header, body)?;
    serde_json::from_slice(&payload).context("docker stats json")
}

fn split_http(raw: &[u8]) -> anyhow::Result<(&[u8], &[u8])> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("docker stats response has no header terminator")?;
    Ok((&raw[..sep], &raw[sep + 4..]))
}

fn status_code(header: &str) -> anyhow::Result<u16> {
    header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .context("docker stats status line")
}

fn decode_http_body(header: &str, body: &[u8]) -> anyhow::Result<Vec<u8>> {
    if header_value(header, "transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"))
    {
        return decode_chunked(body);
    }
    if let Some(len) = header_value(header, "content-length") {
        let len: usize = len.parse().context("content-length")?;
        if body.len() < len {
            anyhow::bail!("docker stats body shorter than content-length");
        }
        return Ok(body[..len].to_vec());
    }
    Ok(body.to_vec())
}

fn header_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim().eq_ignore_ascii_case(name) {
            Some(value.trim())
        } else {
            None
        }
    })
}

fn decode_chunked(mut rest: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .context("chunk size line")?;
        let line = std::str::from_utf8(&rest[..line_end]).context("chunk size")?;
        let size_hex = line.split_once(';').map(|(size, _)| size).unwrap_or(line);
        let size = usize::from_str_radix(size_hex.trim(), 16).context("chunk size")?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            break;
        }
        if rest.len() < size + 2 {
            anyhow::bail!("short chunk");
        }
        out.extend_from_slice(&rest[..size]);
        if &rest[size..size + 2] != b"\r\n" {
            anyhow::bail!("chunk missing crlf");
        }
        rest = &rest[size + 2..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_scales_kibibytes() {
        let text = "\
MemTotal:       1024 kB
MemFree:         100 kB
MemAvailable:    256 kB
Cached:           64 kB
SwapTotal:      2048 kB
SwapFree:          8 kB
AnonPages:       512 kB
";
        let snapshot = parse_meminfo(text).unwrap();
        assert_eq!(snapshot.mem_total_bytes, 1024 * 1024);
        assert_eq!(snapshot.mem_available_bytes, 256 * 1024);
        assert_eq!(snapshot.swap_total_bytes, 2048 * 1024);
        assert_eq!(snapshot.swap_free_bytes, 8 * 1024);
        assert_eq!(snapshot.anon_pages_bytes, 512 * 1024);
        assert_eq!(snapshot.cached_bytes, 64 * 1024);
    }

    #[test]
    fn parse_process_memory_drops_unlimited_limit() {
        let body = serde_json::json!({
            "memory_stats": {
                "usage": 4096,
                "max_usage": 8192,
                "limit": 9223372036854771712u64,
                "stats": {
                    "anon": 3000,
                    "file": 1000,
                    "swap": 20
                }
            }
        });
        let memory = parse_process_memory(&body).unwrap();
        assert_eq!(memory.usage_bytes, Some(4096));
        assert_eq!(memory.anon_bytes, Some(3000));
        assert_eq!(memory.file_bytes, Some(1000));
        assert_eq!(memory.swap_bytes, Some(20));
        assert_eq!(memory.limit_bytes, None);
        assert_eq!(memory.peak_bytes, Some(8192));
    }

    #[test]
    fn parse_process_memory_sums_file_pages_when_file_is_absent() {
        let body = serde_json::json!({
            "memory_stats": {
                "usage": 10,
                "stats": { "active_file": 4, "inactive_file": 6 }
            }
        });
        let memory = parse_process_memory(&body).unwrap();
        assert_eq!(memory.file_bytes, Some(10));
        assert_eq!(memory.anon_bytes, None);
    }

    #[test]
    fn http_json_reads_content_length_body() {
        let raw = b"HTTP/1.0 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}junk";
        let value = http_json(raw).unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn http_json_decodes_chunked_body() {
        let raw =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\n{\"a\":1}\r\n0\r\n\r\n";
        let value = http_json(raw).unwrap();
        assert_eq!(value["a"], 1);
    }

    #[test]
    fn http_json_rejects_error_status() {
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n{}";
        assert!(http_json(raw).is_err());
    }
}
