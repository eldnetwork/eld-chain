# Vendored `tendermint-proto` 0.23.7 (Eld)

## Why this directory exists

eld-chain’s ABCI app talks to Tendermint through crates.io `abci-rs`, which depends on
`tendermint-proto` **0.23.7**. That crate’s `ResponseEndBlock` only has fields 1–3.

Eld adds one wire field so the app can tell the node when the safe summarization height
(the “frontier”) advances:

```text
int64 updated_frontier = 4;   // 0 = no change this block; same type as block height
```

Cargo’s `[patch.crates-io]` replaces a whole package, not a single file. So this tree is a
full copy of Informal Systems’ `tendermint-proto` 0.23.7, with a small Eld delta. The
workspace root pins it with:

```toml
[patch.crates-io]
tendermint-proto = { path = "vendor/tendermint-proto" }
```

That patch applies only to the **0.23.7** line used by `abci-rs`. Workspace `tendermint` /
`tendermint-rpc` **0.40** still use crates.io `tendermint-proto` 0.40.x.

The matching field lives in the Eld Tendermint node repo (`eld-tendermint-rs`) proto. Both
sides must agree on tag 4 for the ABCI socket encode/decode to work.

## What this tree includes

Everything from the published 0.23.7 crate (Apache-2.0), for example:

- `src/lib.rs` and supporting modules (`error`, `serializers`, `google` types)
- Generated prost types under `src/prost/` (ABCI, crypto, types, …)
- Upstream `Cargo.toml`, tests, and license text below

Eld-only changes (also listed in [`ELD_PATCH.md`](ELD_PATCH.md)):

1. `ResponseEndBlock.updated_frontier` (`int64`, protobuf tag 4) in
   `src/prost/tendermint.abci.rs`
2. Drop unused `prelude` re-exports so the crate builds under current rustc with upstream’s
   `#![deny(warnings)]`

Do not hand-edit other prost files unless you are intentionally rebasing this vendor copy.

## Upstream

- crates.io: <https://crates.io/crates/tendermint-proto/0.23.7>
- Source project: <https://github.com/informalsystems/tendermint-rs> (`proto` crate)

## License

Copyright © 2020 Informal Systems

Licensed under the Apache License, Version 2.0 (the "License");
you may not use the files in this directory except in compliance with the License.
You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
