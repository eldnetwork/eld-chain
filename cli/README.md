# eld-cli

`eld-cli` is the command-line client for an Eld node. It is a binary (`publish = false`), not a library.

Parsing and printing live in this crate. Network calls, wallet files, and transaction signing go through [`eld-client`](../client/README.md) (`ChainClient`). Protocol types live in [`eld-common`](../common/README.md).

Results are printed on stdout. Failures are printed on stderr and the process exits 1. Tracing is off unless `RUST_LOG` is set (`RUST_LOG=eld_cli=info`).

Package name: `eld-cli`. Binary: `eld-cli`.

## Setup

From the workspace root:

```sh
cargo run -p eld-cli -- --help
```

The process reads two files from the working directory:

| Path | Purpose |
|---|---|
| `config/config.json` | Tendermint RPC, app REST, and faucet endpoints. Copy [`config/config.json.example`](config/config.json.example). |
| `config/consensus_config.json` | `chain_id` and `fee_config` used when signing. Same file the node loads. |
| `wallets/wallets.json` | Local Ed25519 keys. Gitignored. Never commit this file. |

`--cli-config` replaces `config/config.json`. It does not change the consensus or wallet paths.

Optional `node_url`, `app_url`, and `faucet_url` in the client config override host and port when set.

## Commands

```sh
eld-cli --help
```

| Command | Role |
|---|---|
| `create-wallet` | Create a local signing wallet |
| `list-wallets` | List wallets in `wallets/wallets.json` |
| `get-wallet` | Show one local wallet |
| `remove-wallet` | Delete a local wallet and its private key |
| `transfer` | Sign and commit a transfer |
| `request-faucet` | Ask the dev faucet for tokens |
| `get-account` | Read an account balance and nonce |
| `get-stake-account` | Read a staking account |
| `get-abci-info` | Read Tendermint ABCI info |
| `stake` / `unstake` | Stake or unstake from a local wallet |
| `view-active-validators` | List validators in the current epoch |
| `view-epoch` | Epoch metadata and validator set |
| `get-namespace` / `add-namespace` | Look up or register a namespace |
| `post-pinboard-message` | Post a pinboard message |
| `pinboard-get-post` | Fetch one pinboard post |
| `pinboard-list-by-tag` / `pinboard-list-by-wallet` | Page pinboard posts |
| `get-cado` / `list-cados` | Read one CADO, or list paths |

```sh
eld-cli transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000
eld-cli --cli-config config/config.json get-account 0x1234567890abcdef1234567890abcdef12345678
eld-cli post-pinboard-message my-wallet ./message.txt --content-type text/plain
```

Addresses are `0x` plus 40 hex digits. Amounts are base units and must fit in a coin. Pinboard `--content-type` is `text/plain`, `application/json`, or `image/png`.
