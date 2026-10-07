# eld-faucet

HTTP server that sends test funds from a local wallet. Package `eld-faucet`, binary `eld-faucet`.

`wallets/wallets.json` holds an unencrypted Ed25519 key. It is gitignored. `cargo run` needs that file on disk, with a wallet named `wallet-faucet-1`.

## Run

From this directory:

```sh
cargo run
```

That reads `config/config.json` and `config/consensus_config.json` from the current directory and listens on `faucet_host`:`faucet_port` (`127.0.0.1:8080` in the checked-in config). Signing uses the built-in fee schedule. It is not read from the consensus file.

| Method | Path | Response |
|---|---|---|
| GET | `/health` | body `OK` |
| POST | `/request` | JSON `{ "success", "message" }` for body `{ "address": "0x..." }` |
