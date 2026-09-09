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
