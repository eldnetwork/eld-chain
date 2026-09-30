# Changelog

All notable changes to **eld-chain** (this workspace: `eld-common`, `eld-client`, `eld-cli`, `eld-node`) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Git tags, GHCR image tags, and `Cargo.toml` versions use the same number.
Crates are unpublished (`publish = false`).

## [Unreleased]

### Added

- `eld-cli` — command-line client over `ChainClient` ([cli/README.md](cli/README.md)).
- Grouped `eld-cli` commands (`wallet`, `account`, `tx`, `chain`, `namespace`, `pinboard`, `cado`, `completions`). The old flat verbs stay as hidden aliases.
- `eld-cli` flags `--output text|json`, `--wallets`, `--yes`, and `--dry-run`. Shell completions print to stdout.
- Tag `eld-cli-v*` (for example `eld-cli-v0.0.2`) builds `eld-cli` for Linux x86_64 and arm64, and for macOS arm64 and x86_64, and attaches the binaries to a draft GitHub Release. Publishing that draft is a manual step in the GitHub UI (`.github/workflows/cli.yml`).
- `eld-cli` asks for a node address the first time a command needs one, and for a faucet address only on `tx faucet`. `eld-cli config node` and `eld-cli config faucet` update those addresses later.

### Changed

- `eld-cli` no longer reads `config/consensus_config.json`. Fees used when signing are the built-in defaults. `chain_id` lives in `eld-cli-config.json`.
- `eld-cli` stores config and wallets in the user directories (XDG on Linux, `~/Library/Application Support/eld` on macOS). `--home` is gone. `--cli-config` and `--wallets` still override those paths.
- `eld-cli` is version `0.0.2`. The release tag is `eld-cli-v0.0.2`. The other crates stay `0.0.1`.
- Crate versions are `0.0.1`, the same number as git tag `v0.0.1`.
- `Coin` no longer implements SCALE. Amounts stay on serde (JSON decimal strings and bincode). `eld-common` warns on missing docs for `Address`, `Tx`, `Wallet`, the typed IDs, and `EldError`.

### Fixed

## [0.0.1] - 2026-09-26

First tagged workspace release. Image: `ghcr.io/eldnetwork/eld-chain:v0.0.1`.
This is experimental: protocol constants are local-dev values, not mainnet economics.

### Added

- Workspace crates:
  - `eld-common` — protocol types, validation, `Wallet`, CADO, capacity proofs, pinboard/namespaces ([common/CHANGELOG.md](common/CHANGELOG.md)).
  - `eld-client` — Tendermint RPC, app REST, faucet, `ChainClient`, CWD config, wallet files ([client/CHANGELOG.md](client/CHANGELOG.md)).
  - `eld-node` — Tendermint ABCI application (RocksDB, libp2p, Axum REST).
- Local Docker Compose: single-node and 4-node stacks under `deploy/docker/local/`, plus start/stop scripts.
- GHCR multi-arch release image (`linux/amd64`, `linux/arm64`) via `.github/workflows/image.yml` on `v*.*.*` tags.
- Single-node Compose that pulls `ghcr.io/eldnetwork/eld-chain` and `ghcr.io/eldnetwork/eld-tendermint`.
- CI gate (`./deploy/scripts/ci.sh` and GitHub Actions): fmt, Clippy (warnings as errors), build, test, `cargo audit`, `cargo deny`, gitleaks.
- Dependabot, `CODEOWNERS`, issue/PR templates, `SECURITY.md`.
- Protocol constants loaded from genesis and reloaded from DB on node restart.

### Changed

- GHCR release tags are semver only (no digest suffix on the published tag).
- Compose and deploy scripts read image tags from an env file (no leading spaces so Compose can parse them).
- EC2 deploy workflow and related scripts added under `deploy/`.

### Fixed

- EC2 release env file written without leading spaces so Compose can read image tags.

### Security

- Report consensus, transaction, wallet, or capacity-proof issues via [GitHub Security Advisories](SECURITY.md), not public issues.
- Wallet files are unencrypted Ed25519 keys; do not commit `wallets.json`.

[Unreleased]: https://github.com/eldnetwork/eld-chain/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/eldnetwork/eld-chain/releases/tag/v0.0.1