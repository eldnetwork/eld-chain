# eld-faucet

HTTP server that sends test funds from a local wallet. Package `eld-faucet`, binary `eld-faucet`.

## Keys and data

- `wallets/wallets.json` holds an unencrypted Ed25519 key. The `wallets/` directory is gitignored. Do not commit it.
- Rate-limit state lives in SQLite at `db_path` (default `data/faucet.db`). `faucet/data/` is gitignored.
- In Docker (or any host that restarts the process), mount a volume over `data/` so the SQLite file survives, and mount the wallet file or set `FAUCET_WALLET_PATH` to a secret that is not in the image.

`cargo run` needs the wallet file on disk, with a wallet named `wallet-faucet-1` unless you override the name.

## Config

Reads `config/faucet_config.json` relative to the process working directory. A checked-in sample is in `faucet/config/faucet_config.json` (placeholder host, no secrets). Required fields are `node_host` and `chain_id`. Everything else has defaults.

| Field | Default | Meaning |
|---|---|---|
| `node_host` | (required) | Tendermint RPC host |
| `chain_id` | (required) | Must match Tendermint `status` |
| `bind_host` | `0.0.0.0` | HTTP listen host |
| `bind_port` | `8080` | HTTP listen port |
| `drip_base_units` | `1000000000` | Amount sent per successful drip |
| `address_daily_drips` | `1` | Max drips per recipient address per UTC day |
| `ip_hourly_requests` | `3` | Max requests per peer IP per UTC hour |
| `hot_wallet_reserve` | `10 * drip` | Balance that must remain after a drip |
| `db_path` | `data/faucet.db` | SQLite file for rate limits |
| `wallet_name` | `wallet-faucet-1` | Name of the hot wallet in the wallet file |

Tendermint RPC port is fixed at `26657`. Signing uses the built-in fee schedule.

### Env overrides

| Variable | Overrides |
|---|---|
| `FAUCET_BIND_HOST` | `bind_host` |
| `FAUCET_BIND_PORT` | `bind_port` |
| `FAUCET_DB_PATH` | `db_path` |
| `FAUCET_WALLET_PATH` | wallet file path (default `wallets/wallets.json`) |
| `FAUCET_WALLET_NAME` | `wallet_name` |
| `FAUCET_TRUST_PROXY` | when `1`, use the first `X-Forwarded-For` hop as client IP |
| `FAUCET_CORS_ORIGIN` | allow browser calls from this one origin |

## Run

From this directory:

```sh
cargo run
```

| Method | Path | Response |
|---|---|---|
| GET | `/health` | body `OK` (no I/O) |
| GET | `/ready` | `200 OK` when the wallet is loaded, SQLite answers, and Tendermint `status` chain id matches config; otherwise `503` |
| POST | `/faucet/request` | JSON `{ "success", "message" }` for body `{ "address": "0x..." }`. On success also `tx_hash` and `amount`. Body over 1 KB is rejected. |

CORS is off by default. Client request and disconnect timeouts are 30 seconds; RPC calls used by the faucet share that cap.

## Rate limits

Each successful check counts **one drip** for the requested address and **one request** for the client IP. Caps come from config (defaults above). Both must have room or the call returns `429` and neither counter moves.

- Address limit: after `address_daily_drips` drips to the same `0x…` address in one UTC day, further requests for that address get `429`.
- IP limit: after `ip_hourly_requests` from the same peer IP in the current UTC hour, further requests from that IP get `429`.
- A `429` body is `{ "success": false, "message": "try again later" }` plus `Retry-After` (seconds until the next UTC day, or until the hour window ends).
- IP is the TCP peer address unless `FAUCET_TRUST_PROXY=1`.
- Counters survive a restart in SQLite. A failed broadcast releases the slot. Rows older than two days are purged every hour.
- Transfers are signed one at a time. Before each transfer the faucet checks that its account holds at least `drip_base_units + hot_wallet_reserve`. If not, it returns `503` `faucet empty` and does not spend the daily/IP slot. The balance is cached for 5 seconds; the sign lock covers the check and the transfer.
- A node/network failure returns `503` `node unavailable`; other submit failures return `400` `Error submitting tx`. The response never includes the chain error text.
