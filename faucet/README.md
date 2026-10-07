# eld-faucet

HTTP server that sends test funds from a local wallet. Package `eld-faucet`, binary `eld-faucet`.

`wallets/wallets.json` holds an unencrypted Ed25519 key. It is gitignored. `cargo run` needs that file on disk, with a wallet named `wallet-faucet-1`.

## Run

From this directory:

```sh
cargo run
```

That reads `config/faucet_config.json` (`node_host` and `chain_id`) and listens on `0.0.0.0:8080`. Tendermint RPC port is `26657`. Signing uses the built-in fee schedule.

| Method | Path | Response |
|---|---|---|
| GET | `/health` | body `OK` |
| POST | `/faucet/request` | JSON `{ "success", "message" }` for body `{ "address": "0x..." }` |

## Rate limits

Each successful check counts **one drip** for the requested address and **one request** for the client IP. Caps come from config (defaults below). Both must have room or the call returns `429` and neither counter moves.

| Cap | Scope | Window | Default |
|---|---|---|---|
| `address_daily_drips` | recipient address | UTC calendar day | `1` |
| `ip_hourly_requests` | socket peer IP | UTC hour | `3` |

- Address limit: after `address_daily_drips` drips to the same `0x…` address in one UTC day, further requests for that address get `429` with `Daily faucet limit reached for this address.`
- IP limit: after `ip_hourly_requests` from the same peer IP in the current UTC hour, further requests from that IP get `429` with `Hourly faucet limit reached for this IP.`
- IP is the TCP peer address only (no `X-Forwarded-For` yet).
- Counters are stored in SQLite at `db_path` (default `data/faucet.db`). They survive a restart. A failed broadcast releases the slot. Rows older than two days are purged every hour.
