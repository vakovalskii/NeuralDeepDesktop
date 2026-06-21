# task_07 — Light desktop wrapper (Tauri shell)

> Status: `build` · Depends on: task_01, task_08 · Blocks: task_09

## Goal

Build the **thin** desktop: a Tauri shell with a clean streaming chat + minimal settings that
spawns/supervises the backend (task_06) and talks to Hermes over HTTP+SSE (task_08). No
control-panel reimplementation; the harness stays full underneath.

## Decision: build thin, reuse our skeleton

- **Reuse the `NeuralDeskApp` Tauri shell** (Rust host + React 19 + Vite + Tailwind + the
  already-stripped chat UI + Developer-ID signing/notarization config). It is already a thin
  chat shell on the exact stack Hermes' own `web/` uses, so porting pieces is cheap.
- **Do not** adopt heavy frontends (Open WebUI, LibreChat, Lobe Chat) or the official
  Hermes desktop binary (closed, Nous-Portal credit-gated). **Do not** ship fathah's Electron
  UI (heavy). Borrow *patterns* (SSE chat rendering, slash commands), not the Electron code.

### UI landscape (reference, ranked for our needs)
- `fathah/hermes-desktop` (Electron, MIT) — best Hermes chat UX → copy patterns, not code.
- Official Hermes `web/` (React19/Vite/Tailwind, MIT) — **control panel** pages
  (config/models/skills/sessions) we can port for the "minimal settings".
- `cc-switch` (Tauri/Rust/React, MIT) — provider/config switcher, not a chat client; a
  reference for multi-tool config UX only.
- `agentic-os` (FastAPI + vanilla JS, MIT) — opencode+Hermes+Gemini orchestration; reference
  for the orchestration dashboard idea, not for adoption.
- Open WebUI / LibreChat / Lobe Chat — general LLM chat UIs; **not** agent-harness aware;
  Open WebUI has branding-retention license clauses (check before any reuse — task_10).

## Scope of the wrapper (day one — keep it minimal)

- **Chat:** message input, streaming assistant output (SSE), tool-progress indicators,
  markdown + code highlight, token/cost readout, stop. (Hermes is agentic — render tool steps
  and the ndcode delegation nicely.)
- **Sessions:** list/open/delete/search (Hermes `/api/sessions*`).
- **Settings (minimal):** the NeuralDeep hub base URL + key (one proxy), model pick
  (`/api/model/options|set`), and a backend status/health panel (provisioning progress,
  start/stop). Optionally surface skills toggle (`/api/skills`).
- **First-run:** provisioning progress UI (task_06) + hub credentials entry.

Out of scope day one: messaging gateways, cron, full profile/persona editors, MCP admin —
all reachable later via Hermes' API if wanted.

## Tasks

- [ ] **T7.1** Scaffold the wrapper from the NeuralDeskApp Tauri shell (Rust host + React) in
      this repo; rebrand to Neural Deep; keep the chat shell + signing config.
- [ ] **T7.2** Replace the model-engine layer with the Hermes HTTP+SSE client (task_08):
      send message → stream tokens/tool steps → render. Reuse the existing `StreamMessage`
      rendering where possible.
- [ ] **T7.3** Settings → "Engine" tab: hub base URL + key + model select + backend status.
- [ ] **T7.4** First-run flow: provisioning progress + credentials; gate chat until backend
      healthy.
- [ ] **T7.5** Wire backend lifecycle controls (start/stop/restart, logs) to the Rust host.

## Acceptance criteria

- [ ] App launches, provisions backend, and a user can chat with Hermes (streaming) and watch
      it delegate a coding task to ndcode — through a **thin** UI.
- [ ] Bundle stays light (Tauri ~10 MB shell; weight is in the provisioned harness, not the
      installer).

## Primary sources

- NeuralDeskApp repo (our Tauri shell + signing).
- `github.com/fathah/hermes-desktop` (chat UX patterns), official Hermes `web/` (settings
  pages), `web/src/lib/api.ts` (endpoints).
