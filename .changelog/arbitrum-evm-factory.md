---
forge: minor
cast: minor
anvil: minor
chisel: minor
foundry-evm-core: minor
foundry-evm: minor
foundry-evm-networks: minor
foundry-cheatcodes: minor
foundry-cheatcodes-spec: minor
foundry-cli: minor
foundry-config: minor
foundry-linking: patch
foundry-primitives: minor
forge-script: minor
forge-verify: minor
---

Integrate ArbOS execution with the EVM factory architecture, retaining Stylus
deployment, configuration, tracing, and nested-call support on the updated
Foundry and REVM stack. Preserve existing ArbOS state when applying local Stylus
overrides to a fork. Restore ArbOS initialization and configured Stylus parameters
when resetting Anvil to a fork or local state.
