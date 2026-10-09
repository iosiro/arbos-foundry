---
forge: patch
cast: patch
anvil: patch
chisel: patch
---

Fixed Stylus account-code-hash queries for nonexistent accounts to return zero,
matching Nitro. Existing empty accounts retain their empty-code hash. Added EVM
and Stylus comparison tests for account creation, mutation, and rollback.
