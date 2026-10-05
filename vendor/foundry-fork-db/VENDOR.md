# Vendored fork database

This directory contains `crates/fork-db` from
[foundry-rs/foundry-core](https://github.com/foundry-rs/foundry-core/tree/6ce81d5c491121c22b3912fb91455d4f2875439e/crates/fork-db),
revision `6ce81d5c491121c22b3912fb91455d4f2875439e`, version `0.27.0`.
The upstream MIT and Apache-2.0 licenses are included unchanged.
Repository dprint formatting excludes `vendor/` to preserve upstream file formatting.
Spelling checks exclude the upstream changelog and release-tool configuration; vendored
Rust code and the remaining documentation are still checked.

## Local changes

- Add `BlockHashMode::Rpc` alongside the default `Evm` mode. Native ArbOS execution
  needs canonical L2 RPC hashes from the database, while its `BLOCKHASH` opcode
  reads L1 history from ArbOS storage.
- In RPC mode, use the RPC block number for the exact fork anchor and ancestry,
  without changing the execution block environment or probing for ArbSys.
- Include the hash mode in cache identity and reject cross-mode disk caches,
  including offline loads. Legacy cache metadata defaults to EVM mode.
- Add regression coverage for RPC ancestry, bounds, probe avoidance, and cache
  compatibility. Existing upstream behavior remains the default.
- Expand inherited manifest metadata, dependencies, and lint settings using the
  upstream workspace values, omit the removed `from_iter_instead_of_collect`
  Clippy lint, and disable publishing this vendored package.

The crate is a member of the arbos-foundry workspace and is selected through its
root `[patch.crates-io]` entry. Its tests run with the workspace tests; focused
validation is `cargo test --locked -p foundry-fork-db --all-features`.

## Updating

Compare the complete upstream crate against the recorded revision, update the
vendored files and licenses, and reapply the local changes above. Preserve the
upstream dependency features and metadata when expanding workspace inheritance.
Update this revision record and run the crate tests plus the Arbitrum fork and
Anvil reset regressions before accepting an update. Other foundry-core packages
remain independent upstream dependencies.
