# eld-cli

**Local wallets store unencrypted Ed25519 keys.** Never commit `wallets.json`. Keep the file mode `0600`. The CLI prints name, address, and public key only.

`eld-cli` is the command-line client for an Eld node. It is a binary (`publish = false`), not a library.

Parsing and printing live in this crate. Network calls, wallet files, and transaction signing go through [`eld-client`](../client/README.md) (`ChainClient`). Protocol types live in [`eld-common`](../common/README.md).

Data goes to stdout. Warnings, errors, and logs go to stderr.

Package name: `eld-cli`. Binary: `eld-cli`.

## Install

From the workspace root:

```sh
cargo install --path cli --locked
cargo run -p eld-cli -- --help
```

A tag `eld-cli-v*` (for example `eld-cli-v0.0.1`) builds release binaries and attaches them to a draft GitHub Release. The tarballs are public after that draft is published. Linux assets are `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`; those binaries need `libssl.so.3` (Debian and Ubuntu). macOS is `aarch64-apple-darwin`. That binary is unsigned and not notarized, so Gatekeeper will block a browser download until the quarantine attribute is removed. Each release includes `sha256sums.txt`.

```sh
tag=eld-cli-v0.0.1
asset="${tag}-x86_64-unknown-linux-gnu.tar.gz"
curl -fsSL -O "https://github.com/eldnetwork/eld-chain/releases/download/${tag}/${asset}"
tar -xzf "$asset"
```

## Setup

Start a node with the Docker scripts in the [root README](../README.md). The single-node script publishes Tendermint RPC on `26657` and app REST on `9001`.

The CLI reads three paths. `--home` defaults to `.`. An explicit flag replaces that default.

| Path | Flag | Purpose |
|---|---|---|
| `$home/config/config.json` | `--cli-config`, env `ELD_CLI_CONFIG` | Tendermint RPC, app REST, and faucet endpoints. Copy [`config/config.json.example`](config/config.json.example). |
| `$home/config/consensus_config.json` | `--consensus-config` | `chain_id` and `fee_config` used when signing. |
| `$home/wallets/wallets.json` | `--wallets` | Local Ed25519 keys. Gitignored. |

```sh
mkdir -p config
cp cli/config/config.json.example config/config.json
cp cli/config/consensus_config.json config/consensus_config.json
```

Run those copies from the workspace root. The example faucet endpoint is `127.0.0.1:8080`.

`wallet create`, `wallet list`, `wallet show`, and `wallet remove` need only the wallet file. Every other command needs the client config and a node URL.

Optional `node_url`, `app_url`, and `faucet_url` in the client config override host and port when set.

## Global flags

| Flag | Meaning |
|---|---|
| `--home <DIR>` | Base directory for the three paths above. Default `.`. |
| `--wallets <PATH>` | Wallet file. |
| `--consensus-config <PATH>` | Consensus config file. |
| `--cli-config <PATH>` | Client endpoints file. Env: `ELD_CLI_CONFIG`. |
| `-y`, `--yes` | Skip the `wallet remove` confirmation. |
| `--dry-run` | Print the intended transfer, stake, unstake, namespace add, or pinboard post. Does not broadcast and does not print a tx hash. |
| `--output text\|json` | Default `text`. Env: `ELD_CLI_OUTPUT`. JSON is one document on stdout. |

`wallet remove` prompts `Remove wallet '<name>' and its private key? [y/N]` on a terminal. Without a terminal, pass `--yes` or the command exits 1 and does not delete the key.

Every wallet command prints this on stderr:

`Warning: local wallets store unencrypted Ed25519 keys. Keep wallets.json mode 0600 and never commit it.`

Tracing is off unless `RUST_LOG` is set. Logs go to stderr (`RUST_LOG=eld_cli=info`).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success, including `--help` and `--version`. |
| 1 | Runtime error (config, network, rejected confirmation). |
| 2 | Usage error (unknown command or invalid argument). |

## Commands

Grouped commands are the ones `--help` lists. Amounts are base units. Addresses are `0x` plus 40 hex digits. Pinboard `--content-type` is `text/plain`, `application/json`, or `image/png`. Pinboard `--visibility` is `Public` or `public`.

| Command | Role |
|---|---|
| `wallet create\|list\|show\|remove` | Local signing keys |
| `account get` | Balance and nonce |
| `account stake-get` | Staking account |
| `tx transfer\|stake\|unstake` | Sign and broadcast |
| `tx faucet` | Dev faucet (`tx request-faucet` is the same command) |
| `chain abci-info` | Tendermint ABCI info |
| `chain epoch` | Epoch metadata and validator set |
| `chain validators` | Validators in the current epoch |
| `namespace get\|add` | Look up or register a namespace |
| `pinboard post` | Post a message |
| `pinboard get` | Fetch one post |
| `pinboard list-tag` / `pinboard list-wallet` | Page posts |
| `cado get` / `cado list` | Read one CADO, or list paths |
| `completions bash\|zsh\|fish\|elvish\|powershell` | Shell completion script on stdout |

```sh
eld-cli wallet create my-wallet
eld-cli wallet list
eld-cli --output json wallet list
eld-cli tx faucet 0x1234567890abcdef1234567890abcdef12345678
eld-cli account get 0x1234567890abcdef1234567890abcdef12345678
eld-cli tx transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000
eld-cli --dry-run tx transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000
eld-cli pinboard post my-wallet ./message.txt --content-type text/plain
eld-cli completions bash
```

## Aliases

The old flat names still parse. They are hidden from `--help`.

| Alias | Command |
|---|---|
| `create-wallet` | `wallet create` |
| `list-wallets` | `wallet list` |
| `get-wallet` | `wallet show` |
| `remove-wallet` | `wallet remove` |
| `transfer` | `tx transfer` |
| `request-faucet` | `tx faucet` |
| `get-account` | `account get` |
| `get-stake-account` | `account stake-get` |
| `get-abci-info` | `chain abci-info` |
| `stake` | `tx stake` |
| `unstake` | `tx unstake` |
| `view-active-validators` | `chain validators` |
| `view-epoch` | `chain epoch` |
| `get-namespace` | `namespace get` |
| `add-namespace` | `namespace add` |
| `post-pinboard-message` | `pinboard post` |
| `pinboard-get-post` | `pinboard get` |
| `pinboard-list-by-tag` | `pinboard list-tag` |
| `pinboard-list-by-wallet` | `pinboard list-wallet` |
| `get-cado` | `cado get` |
| `list-cados` | `cado list` |
