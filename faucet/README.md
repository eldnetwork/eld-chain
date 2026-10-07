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
