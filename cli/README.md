# Eld CLI

The Eld CLI is a command-line interface for interacting with the Eld network. Command parsing lives in `src/main.rs`; network, wallet, and transaction logic is provided by [`eld_common`](../../../eld-chain/common) (path dependency on the sibling `eld-chain` repo).

## Setup

From this directory:

```bash
cargo build
cargo run -- --help
```

The crate depends only on `clap`, `tokio`, `tracing`, and `eld_common`. Build with a sibling checkout of `eld-chain` so `../../../eld-chain/common` resolves.

## Wallets

Local key material is stored under `wallets/` (default: `wallets/wallets.json`). That directory is **gitignored** — never commit wallet files. Create wallets with `create-wallet` or copy your own file locally.

## Configuration

The CLI supports different configuration files for connecting to different environments:

- **Default (local):** `config/config.json` — local node at `127.0.0.1`
- **Public testnet (HTTPS):** `config/config-urls-example.json` — `*.eld.network` RPC/API/faucet URLs
- **Custom:** pass any local JSON file with `--cli-config`

Non-local ops configs (e.g. `config-remote*.json` with host IPs) are **not** committed. Keep those files only on your machine under `config/` if you need them.

To specify which config file to use:

```bash
# Use default local config (config/config.json)
cargo run -- get-account 0x123...

# Use public testnet URLs (HTTPS)
cargo run -- --cli-config config/config-urls-example.json get-account 0x123...

# Use a private/local config file (not in git)
cargo run -- --cli-config config/my-deployment.json get-account 0x123...
```

Optional `node_url`, `app_url`, and `faucet_url` in the config override host/port derivation when set. See `config/config-urls-example.json`.

## Content Upload

### Standard Upload (Two-Step Process)

Upload content first, then submit transaction separately:

```bash
cargo run upload-content wallet1 ./mock/mock-content-data-4.json mock-content-4 "application/json"
```

### Atomic Upload with Transaction (Single-Step Process)

Upload content and transaction together atomically. This ensures that content is only stored if the transaction is valid and will be submitted:

```bash
cargo run upload-content-with-tx wallet1 ./mock/mock-content-data-4.json mock-content-4 "application/json"
```

**Note**: The `upload-content-with-tx` command prepares the transaction with the content metadata, signs it, and uploads both together. The node validates the transaction before saving content, ensuring atomicity. This prevents disk space attacks where users upload content without paying.

**Implementation Status**: The endpoint is currently being implemented. The CLI command is ready, but the node endpoint will return an error until full implementation is complete.

### Test content sync

cargo run upload-content wallet1 "./mock/mock-content-data-4.json" "mock-key-4" "application/json"

then

# Get content from node 1
cargo run -- --cli-config config/config-docker-compose-multi-fast-node1.json get-content 0x6ddb7450d754a0dc66a1eb93c682eea6c2ce860b16e62453146f43d7c915fa91

# Get content from node 2
cargo run -- --cli-config config/config-docker-compose-multi-fast-node2.json get-content 0x6ddb7450d754a0dc66a1eb93c682eea6c2ce860b16e62453146f43d7c915fa91

# Get content from node 3
cargo run -- --cli-config config/config-docker-compose-multi-fast-node3.json get-content 0x6ddb7450d754a0dc66a1eb93c682eea6c2ce860b16e62453146f43d7c915fa91

# Get content from node 4
cargo run -- --cli-config config/config-docker-compose-multi-fast-node4.json get-content 0x6ddb7450d754a0dc66a1eb93c682eea6c2ce860b16e62453146f43d7c915fa91

# Post Message
cargo run -- post-pinboard-message wallet1 ./mock/message.txt

