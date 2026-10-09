---
forge: patch
cast: patch
anvil: patch
chisel: patch
---

Fixed Stylus account-code-hash queries for nonexistent accounts to return zero,
matching Nitro. Existing empty accounts retain their empty-code hash. Added EVM
and Stylus comparison tests for account creation, mutation, and rollback.
Historical Anvil state also preserves the distinction between absent and existing
empty accounts.
Storage overrides now materialize missing accounts so their storage survives
historical snapshots and RPC simulation.
Remote empty-account filtering now respects the pre-EIP-161 hardfork rules,
preserving historical transaction replay gas costs.
