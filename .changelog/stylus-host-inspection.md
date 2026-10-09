---
forge: patch
---

Fixed Stylus storage access/state-diff recording and storage observation hooks, isolated base-fee and gas-price overrides, and inspector termination before Wasm execution. Precompile events now propagate immediate `expectEmit` failures instead of only printing them.
