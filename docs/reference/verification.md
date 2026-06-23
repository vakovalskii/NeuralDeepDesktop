---
node_type: report
title: Verification & open questions (Q1–Q10)
status: active
updated: 2026-06-22
links:
  relates_to: [./hermes-backend.md, ./packaging.md, ./architecture.md]
  documents: [../../app/src-tauri/src/lib.rs]
---

# Verification & open questions (task_11)

## Open questions — closed

| # | Question | Answer (this build) |
|---|----------|---------------------|
| Q1 | Hermes chat route + streaming schema | **`POST :8642/v1/chat/completions`, `stream:true` → OpenAI SSE chunks** (`delta.content`), `[DONE]` terminator. `:8642` is the api_server gateway platform. |
| Q2 | Fully local, no Nous account? | **Yes.** `model.provider: custom` + `base_url` (NeuralDeep hub) in `~/.hermes/config.yaml`; no portal login, no credit tier. |
| Q3 | ndcode keeps opencode headless `run`/`serve`? | **Yes.** `~/NeuralDeepCode/packages/ndcode/src/index.ts` registers `RunCommand` (`run [message..]`) and `ServeCommand` (`serve`). |
| Q4 | Non-interactive hub auth for ndcode? | **Yes.** `NEURALDEEP_API_KEY` + `NEURALDEEP_API_BASE` env, skip browser `/login`. |
| Q5 | Hermes config keys for custom OpenAI provider | `model: {provider: custom, base_url, api_key, default}`. Verified live. |
| Q6 | `:8642` stable server mode or internals-coupled? | **Stable.** api_server is a first-class OpenAI-compatible adapter built for external UIs. |
| Q7 | Per-turn abort over local API? | **Yes** — `POST /v1/runs/{run_id}/stop`; the UI "Stop" also aborts the fetch stream. |
| Q8 | macOS notarytool profile name | **Open** — needs user input (only blocks notarization, not dev run). |
| Q9 | Windows Authenticode (EV) cert | **Open** — procurement (only blocks signed Windows installer). |
| Q10 | ndcode `bun build --compile` to one binary? | **Yes (by design)** — Bun app, upstream opencode ships compiled binaries; pin in manifest. |

Only Q8/Q9 remain, and both are ship-time signing concerns — neither blocks the
dev-run goal.

## End-to-end verification gate — dev run

- [x] Local headless Hermes (`hermes gateway`) → `GET :8642/health` → `{"status":"ok","version":"0.14.0"}`.
- [x] One hub base URL + key drives **Hermes** (`config.yaml`) and **ndcode** (env) — same `api.neuraldeep.ru/v1` + `sk-x9…`.
- [x] Chat streams UI → Hermes → **NeuralDeep free tier** → back, rendered in the running dev app (`NEURALDEEP_FREE_TIER_OK`). Screenshot: `../../neuraldeep-desktop-devrun.png`.
- [x] Full harness engaged (≈15k-token system prompt: skills + memory + tools).
- [x] ndcode coding worker: headless `run`/`serve` confirmed from source; Hermes `ndcode` skill authored + installed (`~/.hermes/skills/autonomous-ai-agents/ndcode/SKILL.md`); dev PATH wrapper installed.
- [x] `THIRD_PARTY_LICENSES.md` present; branding is Neural Deep only.
- [x] **Native Tauri desktop app** (`app/src-tauri/`) builds and runs: Rust host spawns/supervises Hermes, streams chat via a `Channel`. Verified live in the native window (`../../tauri-window-chat.png`) — `NEURALDEEP_FREE_TIER_OK` through the Rust path (subtitle "desktop" confirms Tauri transport, not the web proxy).

### Deferred (out of scope for the dev-run goal)

- [ ] Live ndcode coding delegation smoke (`NDCODE_SMOKE_OK`) — requires building/booting the
      ndcode Bun CLI; headless capability already proven from source.
- [x] Tauri native shell (`app/src-tauri/`) — built and running.
- [ ] Signing/notarization of the bundle (`tauri build`, task 09) — needs the notarytool
      profile (Q8) / Windows EV cert (Q9); `tauri:dev` runs unsigned, which is fine for dev.
- [ ] Clean-machine first-run provisioning via `uv` (task_06) — reused the existing
      `~/.hermes` install for the dev run.

## Risks (status)

- Hermes chat API stability — **mitigated**: api_server is a stable, OpenAI-compatible mode;
  transport isolated behind the `/hermes` proxy adapter; Hermes version pinned (v0.14.0).
- Python provisioning fragility — **N/A for dev** (reused install); covered by uv plan for ship.
- Two runtimes (Python + Bun) — ndcode compiles to one native binary; Python via uv.
- Nous-account creep — **closed (Q2)**: account-less local run verified.
- Trademark — **handled (task_10)**: ship as Neural Deep only.
