---
node_type: reference
title: Licensing & commercialization
status: active
updated: 2026-06-22
links:
  documents: [../../THIRD_PARTY_LICENSES.md]
  relates_to: [../decisions/0002-thin-wrapper-full-harness.md]
---

# Licensing & commercialization

The full component chain is permissive (**MIT / Apache-2.0 / PSF**) → **sellable under the
Neural Deep brand**. The authoritative notice file is
[`THIRD_PARTY_LICENSES.md`](../../THIRD_PARTY_LICENSES.md) (ship it in the bundle).

| Component | License | Sell/rebrand? |
|-----------|---------|---------------|
| Hermes Agent | MIT © 2025 Nous Research | ✅ keep notice |
| ndcode (fork of opencode) | MIT | ✅ keep upstream notice |
| fathah/hermes-desktop (patterns) | MIT | ✅ notice if copied |
| uv / python-build-standalone | Apache-2.0 / PSF | ✅ notice |
| Tauri / Bun / React / Vite | MIT (Tauri MIT/Apache) | ✅ notice |

**Conditions:** trademark ≠ code (ship as Neural Deep, not "Hermes"); preserve notices;
model weights are out of scope (served via the hub, not bundled); avoid Open WebUI
(branding-retention clause). See task_10 for the full pass.
