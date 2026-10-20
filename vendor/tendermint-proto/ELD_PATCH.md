# Eld patch

Vendored from crates.io `tendermint-proto` 0.23.7 (Apache-2.0, Informal Systems).

Deltas from upstream:

1. `ResponseEndBlock.updated_frontier` (protobuf tag 4, `uint64`).
   `0` means the frontier did not advance this block. Matches the Eld Tendermint node proto.
2. Drop unused `prelude` re-exports (`Box`, `alloc::vec` macro) so the crate builds
   under current rustc with the crate’s own `#![deny(warnings)]`.
