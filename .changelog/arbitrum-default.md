---
forge: minor
cast: minor
anvil: minor
chisel: minor
foundry-config: minor
foundry-evm-networks: minor
foundry-cli: patch
---

Default local and forked execution to Arbitrum across Forge, Cast tracing, Anvil,
and Chisel. Ethereum execution remains available through explicit network
selection instead of being the implicit fallback. RPC URLs and chain IDs are
unchanged by the execution default.
Explicit CLI network selection takes precedence over environment configuration.
