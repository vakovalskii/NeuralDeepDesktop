# task_01 — Vision & Target Architecture (master)

> Status: `plan` · Owner: TBD · Depends on: none · Blocks: all other tasks

## Goal

Lock the product principle and the end-to-end architecture every other task builds toward.

## Product principle

**Full harness, thin wrapper.** Ship users the *complete* power of Hermes Agent + ndcode,
behind a very light, simple desktop. The wrapper does not reimplement or strip the harness;
it spawns it and gives a clean chat.

- **Hermes is the core/brain** — memory, skills, self-improvement loop, planning, tool use,
  orchestration.
- **ndcode is the coding hand** — Hermes *pulls it in when needed* via a skill (Hermes'
  native "delegate to a coding agent" pattern). Not a peer, not a second engine the user
  toggles.
- **One model proxy** — Hermes and ndcode both call the NeuralDeep hub (OpenAI-compatible).
- **The wrapper is thin** — Tauri shell: streaming chat + minimal settings + backend
  lifecycle. Nothing more on day one.

## Target architecture

```
┌─ Neural Deep.app  (LIGHT WRAPPER — Tauri, ~10 MB) ──────────────────────┐
│  React chat (SSE streaming) + minimal settings                          │
│  Rust host: window, spawn/supervise backend sidecar, sign/notarize      │
│      │  HTTP + SSE (localhost)                                           │
│      ▼                                                                   │
│  FULL HARNESS (provisioned into app-data on first run via uv):          │
│    Hermes Agent (official NousResearch/hermes-agent, headless API)      │
│        │  reads/writes memory, skills, sessions (SQLite FTS5)           │
│        └─ skill "ndcode" ──spawns──▶ ndcode run '<task>'  (coding work) │
│                                                                          │
│  one OpenAI-compatible base URL + key for BOTH                          │
└──────┼───────────────────────────────────────────────────────────────-─┘
       ▼
   NeuralDeep hub  (the single proxy: model routing, keys, cost)
```

## Why this shape (decision record)

- **Hermes ⊃ ndcode (not peers).** Hermes already ships
  `skills/autonomous-ai-agents/opencode/SKILL.md`: it orchestrates a coding agent by
  *spawning a CLI and reading stdout* (`opencode run '<task>'`, or interactive `pty`),
  polling via `process(poll|log|submit)`. ndcode is a fork of that same opencode, so it
  drops into this slot. (Source: hermes-agent repo, opencode SKILL.md.)
- **One real Hermes.** There is a single canonical backend — the official Python
  `NousResearch/hermes-agent`. Every wrapper (fathah, nesquena, agentic-os) runs *that*;
  they differ only in transport. We adopt upstream, don't fork it.
- **Thin wrapper, full power.** The heaviness lives in the (provisioned) Python harness, not
  in the installer. The shell reuses the proven `NeuralDeskApp` Tauri + Developer-ID signing
  skeleton.
- **One proxy.** ndcode already targets the NeuralDeep hub (`/login`, `neuraldeep` provider,
  `NEURALDEEP_HUB`/`NEURALDEEP_API_BASE`). We point Hermes at the same hub → unified models,
  keys, cost. (Source: NeuralDeepCode README.)

## Component inventory & licenses

| Component | Role | Lang/Runtime | License | Decision |
|-----------|------|--------------|---------|----------|
| NousResearch/hermes-agent | core harness | Python (FastAPI, SQLite FTS5) | MIT | adopt upstream, headless |
| vakovalskii/NeuralDeepCode (ndcode) | coding worker | TypeScript / Bun | MIT (fork of sst/opencode) | adopt; restore headless |
| fathah/hermes-desktop | backend layer reference | TypeScript / Electron | MIT | lift backend only |
| NeuralDeep hub | model proxy | (external) | yours | external dependency |
| Tauri shell (from NeuralDeskApp) | wrapper | Rust + React | ours | reuse |

All sellable under our own brand — see `task_10.md`.

## Acceptance criteria for "vision locked"

- [ ] Architecture diagram above agreed.
- [ ] "Hermes core, ndcode pulled-in tool, one proxy, thin wrapper" confirmed as the model.
- [ ] Mac + Windows scope confirmed (see `task_09.md`).
- [ ] External NeuralDeep proxy confirmed as the single gateway (see `task_05.md`).

## Open questions (tracked in task_11)

- Exact Hermes chat transport (port/route) for the wrapper — `task_02`/`task_04`/`task_08`.
- Whether ndcode still has upstream opencode's headless `run`/`serve` — `task_03`.
- Whether Hermes can run fully local (no Nous account) when models come from our proxy —
  `task_02`/`task_05`.

## Primary sources

- `github.com/NousResearch/hermes-agent` (README, `web/src/lib/api.ts`,
  `skills/autonomous-ai-agents/opencode/SKILL.md`, LICENSE = MIT)
- `github.com/vakovalskii/NeuralDeepCode` (README — fork of sst/opencode, MIT)
- `github.com/fathah/hermes-desktop` (MIT; SSE chat via local Hermes API)
- `opencode.ai/docs` (server + SDK), `github.com/sst/opencode`
