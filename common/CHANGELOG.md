# Changelog

All notable changes to `eld-common` are documented here.

## [Unreleased]

### Changed

- Removed the unused SCALE `Encode` / `Decode` impl on `Coin` and the `parity-scale-codec` 1.3 dependency. JSON and bincode still go through serde.
- Documented `Address`, `Tx`, `Wallet`, and `EldError`, and enabled `missing_docs` for those types and the typed IDs.

## [0.0.1] - 2026-09-26

First experimental release as a library crate inside `eld-chain` (not published to crates.io).

### Added

- Protocol types: `Address`, `Coin`, transactions, CADO, capacity proofs, pinboard/namespace types.
- `Wallet` Ed25519 signing identity and transaction validation.
- `SlotAllocator` and on-disk capacity slot maps.
- [TYPE_DESIGN.md](TYPE_DESIGN.md) for hex and typed ID conventions.

Wire format and validation match the current node and client crates in this workspace; this is not a frozen mainnet spec.
