# task_02 — Hermes Agent core: internals, API surface, run model

> Status: `research` (mostly done) + `build` · Depends on: task_01 · Blocks: task_06, task_08

## Goal

Know exactly what the Hermes backend is, how to run it headless, what API a custom desktop
client can use, and how its memory/skills/orchestration work — so the sidecar (task_06) and
transport (task_08) are built on facts, not guesses.

## What Hermes is (research findings)

- Open-source **autonomous agent by Nous Research**, MIT. Self-improving: after complex
  tasks it writes reusable **skills** (procedural memory). Persistent memory + FTS5 session
  search. 200+ models via OpenRouter/Nous Portal/OpenAI/Anthropic/**custom endpoints**.
- **Mostly Python** (~82% Python, ~13% TypeScript). Installs to `~/.hermes` (macOS/Linux) or
  `%LOCALAPPDATA%\hermes` (Windows) via a managed installer
  (`curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash`).
- Repo layout: `agent/` (core), `hermes_cli/` (CLI), `web/` (React 19 + Vite + Tailwind v4
  **management dashboard**), `gateway/` (Telegram/Discord/Slack/WhatsApp/Signal/Email),
  `tui_gateway/` + `ui-tui/` (terminal UIs), `skills/`, `tools/` (40+), `plugins/`,
  `providers/`, `docker/`, `docs/`.

## How it runs / how to drive it (the critical part)

- **Web/management backend:** `python -m hermes_cli.main web` → **FastAPI on `127.0.0.1:9119`**.
  Production build of `web/` is served by the same FastAPI server; dev proxies `/api` →
  `:9119`. (Source: `web/src/lib/api.ts`, `web/` README.)
- **The official `web/` is a CONTROL PANEL, not a chat client.** `web/src/lib/api.ts` has a
  large REST surface (below) but **no chat send / token stream**. Chat happens in CLI/TUI,
  and the local-API chat surface is **not yet a stable public API** (decoupling tracked in
  hermes-agent issues #1925, #2491). There is WebSocket infra (`buildWsUrl()`,
  e.g. `/api/plugins/kanban/events`).
- **A working chat-over-local-API path exists**: `fathah/hermes-desktop` routes chat through
  a local Hermes API at **`127.0.0.1:8642` with SSE**. That proves Hermes chat can be driven
  over local HTTP+SSE. Confirming the exact route is `task_04`.

### Official FastAPI `:9119` REST surface (from `web/src/lib/api.ts`)

Auth: loopback mode injects `window.__HERMES_SESSION_TOKEN__` (header
`X-Hermes-Session-Token`); gated mode uses cookie `hermes_session_at` + `/api/auth/ws-ticket`.

Endpoint groups (selected, full list in source):
- **Sessions:** `GET/DELETE/PATCH /api/sessions`, `/api/sessions/{id}/messages`,
  `/api/sessions/search` (FTS5), `/api/sessions/stats`, export, prune, bulk-delete.
- **Config:** `GET/PUT /api/config`, `/api/config/raw` (YAML), `/api/config/schema`,
  `/api/config/defaults`.
- **Env vars:** `GET/PUT/DELETE /api/env`, `POST /api/env/reveal`.
- **Models:** `GET /api/model/info`, `GET /api/model/options`, `POST /api/model/set`,
  `GET /api/model/auxiliary`.
- **Skills:** `GET /api/skills`, `PUT /api/skills/toggle`, `GET/PUT /api/skills/content`,
  `POST /api/skills`, hub install/search/preview/scan.
- **Toolsets, Profiles (incl. `/soul` = system prompt), Cron, Memory
  (`GET /api/memory`, `PUT /api/memory/provider`, `POST /api/memory/reset`), MCP servers,
  Credentials pool, Gateway lifecycle (`/api/gateway/start|stop|restart`), Analytics,
  Status (`/api/status`).**

→ This API gives us **everything for settings/config/models/skills/sessions** in the wrapper.
The **chat stream** is the one piece to wire from the non-`web/` path (task_04/task_08).

## Memory / skills / learning loop

- Skills = "agent-curated memory with periodic nudges", stored in `~/.hermes/skills/`,
  auto-created after complex tasks, self-improving, compatible with the agentskills.io
  standard. Manageable via `/api/skills*`.
- Memory: pluggable providers (builtin + Honcho/Mem0/RetainDB/Supermemory), FTS5 session
  search. Manageable via `/api/memory*`.
- **opencode orchestration is itself a skill** (`skills/autonomous-ai-agents/opencode/`) —
  see task_03 for how we retarget it to ndcode.

## Tasks

- [ ] **T2.1** Provision Hermes headless locally and confirm `python -m hermes_cli.main web`
      serves `:9119`; capture exact start command, ports, and required env.
- [ ] **T2.2** Determine whether Hermes runs **fully local with NO Nous account** when models
      are supplied by our proxy (config/env only). Document the minimal config.
- [ ] **T2.3** Confirm custom **OpenAI-compatible base URL + key** is settable via
      `/api/config` (or `config.raw` YAML / `NDC_*`-style env). Cross-ref task_05.
- [ ] **T2.4** Map the chat path: confirm whether chat is exposed on `:9119` (some route) or
      only via the fathah `:8642` SSE path (task_04). Document request + SSE event shapes.
- [ ] **T2.5** Decide auth handling for a loopback desktop client (session token injection vs
      `--insecure` loopback). Document.

## Acceptance criteria

- [ ] A documented, reproducible command to start a **headless, local, account-less** Hermes
      that talks to our proxy.
- [ ] Documented chat transport (endpoint + streaming format) the wrapper will use.
- [ ] Confirmed config path to point Hermes at the NeuralDeep hub.

## Primary sources

- `github.com/NousResearch/hermes-agent` — README, `web/src/lib/api.ts`,
  `hermes_cli/`, `docs/`.
- `hermes-agent.nousresearch.com/desktop` (official desktop = closed binary, Nous-Portal
  credit tiers — NOT our base; see task_01).
- Issues #1925, #2491 (stable-API decoupling) — confirm current state.
