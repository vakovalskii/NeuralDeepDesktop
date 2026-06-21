# task_06 — Backend packaging (uv + standalone CPython + sidecar)

> Status: `build` · Depends on: task_02, task_03, task_04 · Blocks: task_09

## Goal

Package "the full harness" so it's **easy to ship** on Mac + Windows while keeping the
**installer light**: bundle only native/static bits; provision the heavy Python Hermes on
first run, deterministically.

## The packaging problem (research)

| Component | Runtime | Packaging difficulty |
|-----------|---------|----------------------|
| Hermes (core) | **Python** (FastAPI, SQLite FTS5, many deps) | 🔴 hard — runtime + deps |
| ndcode (worker) | **Bun** / native | 🟢 `bun build --compile` → 1 binary (task_03) |
| Proxy | external NeuralDeep hub | 🟢 nothing to bundle (task_05) |

→ The only hard part is the Python harness. Three strategies were considered:

- **A. Bundle standalone CPython** (python-build-standalone via `uv`) + venv in app Resources
  — self-contained, **offline**, deterministic; heavier per-OS build.
- **B. Install-on-first-run** into app-data (guided installer; this is fathah's model) —
  lightest installer; needs network on first launch.
- **C. Remote backend** (URL+key) — thinnest; needs a host.

## Decision

**Hybrid A+B via `uv`** — best fit for "light wrapper + full harness + easy package":

1. **Bundle (small):** Tauri shell + native `ndcode` binary + a tiny **`uv`** binary
   (per-target). No Python interpreter stuffed into the installer.
2. **First run (provision):** the sidecar runs `uv` to fetch a **standalone CPython** and
   `uv pip install` the **full** Hermes into app-data (`~/Library/Application Support/NeuralDeep`
   on macOS, `%APPDATA%\NeuralDeep` on Windows). Deterministic, fast, identical across OSes;
   cacheable for offline. (Optional later: pre-seed the CPython + wheels into Resources for a
   fully offline first run = upgrade to pure strategy A.)
3. **Run:** the sidecar spawns Hermes **headless** as a local API (task_02/task_04) and keeps
   it supervised.

Result: backend = **1 native binary (ndcode) + 1 uv-managed Python (full Hermes) + a thin
launcher**; proxy external. Installer stays light; users still get the complete harness.

## Tasks

- [ ] **T6.1** Choose the sidecar/launcher home: reuse the Tauri Rust host
      (`src-tauri`, lifted from NeuralDeskApp) to spawn/supervise processes, OR a tiny Node/Bun
      sidecar. Decision likely: **Rust host spawns directly** (fewest moving parts).
- [ ] **T6.2** Vendor per-target `uv` binaries; wire a Tauri `externalBin` for `uv`
      (`bin/uv-<triple>`), like opencode was bundled in NeuralDeskApp.
- [ ] **T6.3** Implement first-run provisioning: `uv python install` (pinned CPython) +
      `uv pip install hermes…` (or `uv tool install`) into app-data; idempotent; progress
      surfaced to the UI; failure/retry handling; offline detection.
- [ ] **T6.4** Bundle `ndcode` as a native `externalBin` (`bin/ndcode-<triple>`) from
      `bun build --compile` (task_03). Ensure it's on the Hermes process `PATH` (task_08).
- [ ] **T6.5** Implement backend lifecycle: start headless Hermes API, health-check
      (poll `/api/status` or chat endpoint), restart on crash, clean shutdown on app exit.
- [ ] **T6.6** Pin versions: a manifest of {Hermes version, ndcode commit, CPython version,
      uv version} for reproducible builds.
- [ ] **T6.7** Define app-data layout + first-run vs upgrade (re-provision on Hermes version
      bump; preserve user memory/skills/sessions).

## Acceptance criteria

- [ ] Fresh machine → install app → first run provisions full Hermes via uv → headless API up
      → chat works, ndcode delegation works — on **both** macOS and Windows.
- [ ] Installer size stays modest (shell + ndcode + uv; no giant Python blob unless offline
      mode chosen).
- [ ] Backend survives crashes (supervised) and shuts down cleanly.

## Primary sources

- `astral.sh/uv` docs — `uv python install`, `uv pip`, standalone CPython
  (python-build-standalone).
- `github.com/fathah/hermes-desktop` — its provisioning approach (task_04).
- Tauri v2 `externalBin` / sidecar docs.
- NeuralDeskApp repo — existing `externalBin` bundling + `scripts/build-sidecar.cjs` pattern
  (reuse).
