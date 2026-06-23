---
node_type: reference
title: Packaging, provisioning & signing
status: active
updated: 2026-06-22
links:
  documents: [../../app/src-tauri/tauri.conf.json, ../../app/src-tauri/src/lib.rs]
  relates_to: [./architecture.md, ./verification.md]
  depends_on: [../decisions/0002-thin-wrapper-full-harness.md]
---

# Packaging, provisioning & signing notes (tasks 06 + 09)

> For the dev-run goal the backend is **reused in place** (`~/.hermes`). This file records
> the ship-path: how a clean machine would provision the same stack, plus signing.

## Backend provisioning (task_06) — hybrid A+B via `uv`

The only hard-to-package piece is the Python Hermes harness. Decision: bundle small native
bits, provision Python on first run.

**Bundle (small):** Tauri shell + native `ndcode` binary (`bin/ndcode-<triple>`) + a tiny
`uv` binary (`bin/uv-<triple>`) as Tauri `externalBin`. No Python interpreter in the installer.

**First run (provision)** into app-data
(`~/Library/Application Support/NeuralDeep` on macOS, `%APPDATA%\NeuralDeep` on Windows):

```bash
uv python install 3.11            # pinned standalone CPython (python-build-standalone)
uv pip install hermes-agent…      # full Hermes into the app-data venv
```

Idempotent; progress surfaced to the UI; offline-detect + retry. (Optional later: pre-seed
CPython + wheels into Resources for a fully offline first run = pure strategy A.)

**Run / lifecycle (T6.5):** Rust host spawns `hermes gateway` headless, health-checks
`GET :8642/health`, restarts on crash, clean shutdown on quit. `ndcode` must be on the
Hermes process `PATH` (Hermes spawns it as a grandchild via the `ndcode` skill).

**ndcode build (T6.4 / Q10):** `bun build --compile` → one self-contained native binary
per target (ndcode is a Bun app; upstream opencode ships exactly this way). The local repo
`~/NeuralDeepCode` retains `RunCommand`/`ServeCommand` so the compiled binary is headless-capable.

**Version manifest (T6.6)** for reproducible builds:

| Component | Pin (this machine) |
|-----------|--------------------|
| Hermes Agent | v0.14.0 (git) |
| ndcode | local `~/NeuralDeepCode` (fork of sst/opencode) |
| CPython | 3.11.10 (venv) / 3.11 pinned for `uv` |
| uv | 0.6.1 |
| Bun | 1.3.14 |

### Dev shortcut (what we actually used)

Backend already provisioned at `~/.hermes` → we skip first-run and just
`hermes gateway`. `ndcode` resolved via a dev wrapper `~/.local/bin/ndcode` that execs
`bun run --cwd ~/NeuralDeepCode/packages/ndcode src/index.ts`.

## Signing & notarization (task_09) — deferred (not needed for dev run)

Reuse the NeuralDeskApp macOS runbook:

- **Developer ID Application:** `Valeriy Kovalsky (A933C2TJXU)` (in keychain).
- Sign with hardened runtime (`--options runtime`) + timestamp; entitlements `allow-jit`,
  `allow-unsigned-executable-memory`, `disable-library-validation`.
- Sign **every** nested binary: app, host, `ndcode`, `uv`, bundled python.
- Notarize `.app` + `.dmg`: `ditto -c -k --keepParent` → `xcrun notarytool submit --wait`
  → `xcrun stapler staple`; DMG must be UDIF (`hdiutil convert -format UDZO`).
- Verify `spctl -a -vvv -t exec App.app` → "Notarized Developer ID".

**Open blockers (→ task_11):**
- **Q8** macOS `notarytool` keychain-profile name — needs user input.
- **Q9** Windows Authenticode (EV) cert — procurement.

First-run provisioning writes to user-writable app-data (outside the signed `.app`), so it
does not break the signature / Gatekeeper.
