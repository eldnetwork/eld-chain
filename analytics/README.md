# eld-analytics

Sidecar that periodically measures disk usage for Tendermint homes and Eld ABCI
data volumes, stores each run in RocksDB, and serves a small REST API.

## Endpoints

| Path | Description |
|------|-------------|
| `GET /health` | Liveness |
| `GET /analytics/disk/latest` | Most recent `CollectionRun` |
| `GET /analytics/disk/history?limit=N` | Newest-first history (default 24) |

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
| `RUST_LOG` | — | Tracing filter |

Each measurement gives up after 30 seconds and logs an error. Directory sizes that run out of time are skipped. Largest-file scans and RocksDB inventories keep what they already finished, including when a RocksDB read fails. The rest of the run is still stored.

Each `CollectionRun` stores `block_height` from Tendermint `status`
(`sync_info.latest_block_height`), the same field the block explorer uses.

On the tm-rs local stack, enable with:

```sh
./deploy/scripts/docker/local/cluster/cluster-start-without-history.ghcr.tm-rs.sh --with-analytics
```

Combinable with `--with-faucet`. Host port `9010`.
