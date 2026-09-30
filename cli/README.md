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

A tag `eld-cli-v*` (for example `eld-cli-v0.0.2`) builds release binaries and attaches them to a draft GitHub Release. The tarballs are public after that draft is published. Linux assets are `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`; those binaries need `libssl.so.3` (Debian and Ubuntu). macOS assets are `aarch64-apple-darwin` and `x86_64-apple-darwin`. Those binaries are unsigned and not notarized, so Gatekeeper will block a browser download until the quarantine attribute is removed. Each release includes `sha256sums.txt`.

```sh
tag=eld-cli-v0.0.2
asset="${tag}-x86_64-unknown-linux-gnu.tar.gz"
curl -fsSL -O "https://github.com/eldnetwork/eld-chain/releases/download/${tag}/${asset}"
tar -xzf "$asset"
```

## Setup

Start a node with the Docker scripts in the [root README](../README.md). The single-node script publishes Tendermint RPC on `26657` and app REST on `9001`.

The CLI stores two files under the user account. They are not next to the binary and not in the current directory.

| Path | Flag | Purpose |
|---|---|---|
| Linux: `~/.config/eld/eld-cli-config.json` (`$XDG_CONFIG_HOME/eld/` when that variable is set). macOS: `~/Library/Application Support/eld/eld-cli-config.json`. | `--cli-config`, env `ELD_CLI_CONFIG` | Node address, optional faucet address, and `chain_id`. |
| Linux: `~/.local/share/eld/wallets.json` (`$XDG_DATA_HOME/eld/` when that variable is set). macOS: `~/Library/Application Support/eld/wallets.json`. | `--wallets` | Local Ed25519 keys. |

`wallet` commands and `completions` stay offline. They do not read the config file and they do not prompt. Query commands (`account`, `chain`, `namespace get`, `pinboard` reads, `cado`, `tx faucet`) do not open the wallet file. The wallet file is opened only when signing: `tx transfer`, `tx stake`, `tx unstake`, `namespace add`, and `pinboard post`.

The first command that talks to a node (`account`, `tx`, `chain`, `namespace`, `pinboard`, `cado`) asks on a terminal when the file is missing or has no node address:

```text
Node address (IP or URL):
```

An IP or hostname is stored as `node_host` with Tendermint port `26657`. `host:port` uses that port instead. App REST uses the same host on port `9001`. An `http://` or `https://` URL is stored as `node_url`, and app REST uses that host on port `9001`. Faucet fields are left empty. `faucet_end_point` stays `/faucet/request`.

The CLI then reads `chain_id` from Tendermint `GET /status`. If the node does not answer, the address is still saved, `chain_id` stays empty, and the CLI says so. The next command that signs a transaction tells you to run `eld-cli config node` again once the node is up.

Without a terminal, a missing node address exits 1 and prints `eld-cli config node <ip-or-url>`.

`tx faucet` (and the `request-faucet` alias) is the only command that asks for a faucet:

```text
Faucet address (IP or URL):
```

An IP or hostname uses port `8080` and `/faucet/request`. A URL is stored as `faucet_url`. Without a terminal, it exits 1 and prints `eld-cli config faucet <ip-or-url>`.

`eld-cli config node <ip-or-url>` rewrites the node fields and refreshes `chain_id`. Faucet fields already in the file stay. `eld-cli config faucet <ip-or-url>` rewrites only the faucet fields. Both take the address as an argument and create the parent directory when needed.

```sh
eld-cli config node 127.0.0.1
eld-cli config node https://rpc.example.com
eld-cli config faucet 127.0.0.1
eld-cli config faucet https://faucet.example.com
```

[`config/eld-cli-config.json.example`](config/eld-cli-config.json.example) shows the fields. Signing uses the built-in fee schedule. It is not read from a file.

Optional `node_url`, `app_url`, and `faucet_url` override host and port when set.

## Global flags

| Flag | Meaning |
|---|---|
| `--wallets <PATH>` | Wallet file. Overrides the platform path above. |
| `--cli-config <PATH>` | Client endpoints file. Overrides the platform path above. Env: `ELD_CLI_CONFIG`. |
| `-y`, `--yes` | Skip the `wallet remove` confirmation. |
| `--output text\|json` | Default `text`. Env: `ELD_CLI_OUTPUT`. JSON is one document on stdout. |

`wallet remove` prompts `Remove wallet '<name>' and its private key? [y/N]` on a terminal. Without a terminal, pass `--yes` or the command exits 1 and does not delete the key.

`reset` prints both file paths and warns that the wallet file holds unencrypted private keys, then prompts `Delete these files? [y/N]`. Without a terminal, pass `--yes` or the files stay.

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
| `pinboard list-tag` / `pinboard list-by-address` | Page posts |
| `cado get` / `cado list` | Read one CADO, or list paths |
| `config node` / `config faucet` | Set the node or faucet address |
| `reset` | Delete the client config and wallet file |
| `completions bash\|zsh\|fish\|elvish\|powershell` | Shell completion script on stdout |

```sh
eld-cli wallet create my-wallet
eld-cli wallet list
eld-cli --output json wallet list
eld-cli tx faucet 0x1234567890abcdef1234567890abcdef12345678
eld-cli account get 0x1234567890abcdef1234567890abcdef12345678
eld-cli tx transfer my-wallet 0x1234567890abcdef1234567890abcdef12345678 1000
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
| `pinboard-list-by-wallet` | `pinboard list-by-address` |
| `get-cado` | `cado get` |
| `list-cados` | `cado list` |
