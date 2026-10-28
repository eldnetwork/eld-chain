# eld-analytics

Sidecar that periodically measures disk usage and memory for Tendermint homes and
Eld ABCI data volumes, stores each run in RocksDB, and serves a small REST API.

## Endpoints

| Path | Description |
|------|-------------|
| `GET /health` | Liveness |
| `GET /analytics/usage/latest` | Most recent `CollectionRun` (disk and memory) |
| `GET /analytics/usage/history?limit=N` | Newest-first history (default 24) |

## Environment

| Variable | Default | Meaning |
|----------|---------|---------|
| `ANALYTICS_DB_PATH` | `/data/analytics.rocksdb` | Sidecar store |
| `ANALYTICS_LISTEN` | `0.0.0.0:9002` | HTTP bind |
| `FIRST_COLLECTION_DELAY_SECS` | `180` | Wait after start before first sample |
| `COLLECTION_INTERVAL_SECS` | `3600` | Period between samples. Each tick starts a worker and does not wait for the previous one. |
| `NODE_IDS` | `1,2,3,4` | Node id list |
| `TENDERMINT_PATH_PREFIX` | `/tendermint-` | Mount prefix for TM homes |
| `TENDERMINT_DATA_SUBDIR` | `data` | Subdir under each TM home |
| `ABCI_PATH_PREFIX` | `/eld-` | Mount prefix for ABCI data |
| `TENDERMINT_RPC_URL` | `http://tendermint-1:26657` | Tendermint RPC for `latest_block_height` |
| `TENDERMINT_RPC_HOST_PREFIX` | `tendermint-` | Per-node mempool RPC host: `http://{prefix}{id}:{port}` |
| `TENDERMINT_RPC_PORT` | `26657` | Per-node mempool RPC port |
| `DOCKER_SOCKET` | `/var/run/docker.sock` | Docker API socket for per-container memory |
| `TENDERMINT_CONTAINER_PREFIX` | `tendermint-` | Container name prefix for Tendermint memory |
| `ABCI_CONTAINER_PREFIX` | `eld-app-` | Container name prefix for ABCI memory |
| `RUST_LOG` | — | Tracing filter |

Each folder size gives up after 30 seconds and is stored as `0`. The rest of the run is still stored.

Per node, disk collection is folder sizes only. Tendermint: `data/`, `data/tx_index.db`, and `data/evidence.db`. ABCI: the data directory, `rocksdb/`, and `capacity/`. The live RocksDB is not opened.

Each `CollectionRun` stores `block_height` from Tendermint `status`
(`sync_info.latest_block_height`), the same field the block explorer uses.

It also stores memory from the same hourly tick:

- `vm_memory` from `/proc/meminfo`: total, available, swap, anonymous pages, and page cache.
- `tendermint_memory` and `abci_memory`: cgroup usage, anonymous bytes, file cache, swap, limit, and peak from the Docker stats API. A missing socket or a missing container leaves those fields empty and the rest of the run is still stored.
- `mempool_txs` and `mempool_bytes` on each Tendermint row, from that node's `num_unconfirmed_txs`.

The local compose file mounts the Docker socket and adds group `0` so the sidecar user can open it. The socket can start and stop every container on the machine.

A git tag `eld-analytics-v*` (for example `eld-analytics-v0.0.4`) publishes `ghcr.io/eldnetwork/eld-analytics` with that same tag. On the tm-rs local stack, enable with:

```sh
./deploy/scripts/docker/local/cluster/cluster-start-without-history.ghcr.tm-rs.sh --with-analytics
```

Combinable with `--with-faucet`. Host port `9010`.
