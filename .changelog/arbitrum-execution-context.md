---
forge: patch
cast: patch
anvil: patch
chisel: patch
forge-script: patch
forge-verify: patch
foundry-config: patch
foundry-cheatcodes: patch
foundry-evm: patch
foundry-evm-core: patch
---

Preserve Arbitrum execution settings across nested transactions, forks, replay, and cached Chisel sessions. Honor Stylus cache policy and configuration keys, and support value-bearing deployment with matching CREATE2 initcode. Fix ArbSys fork calls, Anvil precompile overrides and simulation dispatch, and gas estimation including L1 poster charges.

Reset fork-local writes and refresh account information when rolling inactive forks, while retaining setup storage and persistent accounts. Preserve block-local Stylus gas discounts across isolated transactions and match Nitro's minimum recent-cache capacity.

Decode native Arbitrum RPC transactions with canonical hash validation and replay system prefixes with the L2 parent-block identity. Share Cast's entry point between binary names so JSON errors retain the same structured output.

Carry the RPC block identity and accumulated block-local context through bytecode verification replay, including native start-block transactions and the target deployment.

Read EVM `BLOCKHASH` from ArbOS's L1 history rather than the L2 block-hash database, preserving the opcode's gas cost. Fetch canonical L2 RPC hashes for ArbSys across Forge fork rolls and Anvil resets, keeping cached hash domains distinct. Keep `vm.roll` and `vm.setBlockhash` overrides separate from L2 history across forks, snapshots, and isolated calls.

Include the fork database in the source tree so Arbitrum hash handling builds without a separate patched dependency checkout.

Declare Rust 1.96 as the minimum version required by the pinned compiler and build-metadata dependencies.

Make explicit dependency updates check out the revision recorded in the lockfile instead of leaving the Git submodule at a different commit.

Keep Forge and Anvil disk caches in separate files for each hash domain, preserving legacy flat cache files without blocking new cache writes.

Include all fork-cache layouts in cache listings and remove both legacy files and their compatibility directories when clearing a block cache.

Restore the EVM-visible block number from Anvil snapshots independently of the RPC block height, preserving Arbitrum's L1/L2 distinction after a revert.

Derive opcode activation, transaction gas rules, and precompiles from persisted ArbOS versions across state replacement and upgrades, and reject unsupported future versions. Reject `BLOBBASEFEE` on Arbitrum, matching Nitro even after Cancun activation.

Honor contract- and function-level Stylus settings without discarding setup state or leaking overrides between tests, including isolated execution and fuzzing.
