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
- `eld-cli` flags `--output text|json`, `--home`, `--wallets`, `--consensus-config`, `--yes`, and `--dry-run`. Shell completions print to stdout.

### Changed

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