# task_04 — fathah/hermes-desktop: backend layer to lift

> Status: `research` (source-read pending) · Depends on: task_02 · Blocks: task_06, task_08

## Goal

Extract the *backend layer* (not the UI) from `fathah/hermes-desktop` — the proven, MIT code
for (a) provisioning/managing a local Hermes and (b) streaming chat over a local API — so we
don't reinvent it. We drop its Electron UI; we keep its know-how.

## What fathah/hermes-desktop is (research findings)

- "Hermes One" — a **community-maintained native desktop app** (Electron) for installing,
  configuring, and **chatting** with Hermes Agent. MIT. ~12.5k stars, ~1.4k forks, v0.6.35,
  39+ releases, 1000+ commits — actively maintained. (Source: repo page.)
- **Full streaming chat UI via SSE** — tool-progress indicators, markdown, syntax highlight,
  token/cost, 22 slash commands. This is the chat surface the official `web/` lacks.
- **Backend handling:** installs + manages Hermes locally in `~/.hermes` (with dependency
  resolution / guided first-run installer) **or** connects to a **remote Hermes API by
  URL + key**. Routes chat through **`http://127.0.0.1:8642` with SSE**. SQLite (FTS5) for
  session history; config in `~/.hermes` YAML + `.env`.
- Multi-provider LLM incl. **local endpoints** (so our proxy fits). 14 toolsets, 16 messaging
  gateways, memory providers, cron, etc. — i.e. it exposes the *full* harness.
- Described as **pluggable agent backend** (swap Hermes for another agent).

## Why lift it (not fork the whole thing)

- It already solved our two hardest backend unknowns: **provisioning Hermes** and **chat SSE
  transport**. MIT → we can copy.
- We do **not** want its Electron shell (heavy; we want a thin Tauri wrapper — task_07).
- Risk: its chat path may reach into Hermes internals on a non-stable API (same fragility as
  nesquena/hermes-webui). Confirm how stable/portable it is.

## Tasks (source reading — be concrete)

- [ ] **T4.1** Clone `fathah/hermes-desktop`. Locate the **provisioning** code: how it
      installs Hermes into `~/.hermes`, resolves Python deps, and detects an existing install.
      Note Python/uv/pip usage we can mirror (task_06).
- [ ] **T4.2** Locate the **chat transport**: what exact local endpoint/route it POSTs a
      message to and reads the **SSE** stream from (the `:8642` server). Capture: request
      shape, SSE event types (token deltas, tool progress, usage, done), and how `:8642` is
      started (does the desktop spawn a Hermes server subprocess? which command?).
- [ ] **T4.3** Determine whether `:8642` is (a) an official Hermes server mode, (b) a small
      proxy the desktop runs in front of Hermes, or (c) direct module import. This decides how
      portable the pattern is.
- [ ] **T4.4** Capture the **remote mode** (URL + API key) contract — useful as our "connect
      to a hosted Hermes" option and as a test harness.
- [ ] **T4.5** Capture the **model/provider config** path it uses to point Hermes at a custom
      endpoint (cross-ref task_05).
- [ ] **T4.6** Produce `docs/hermes-backend-notes.md` in our repo: the distilled, citeable
      "how to provision + how to chat-stream" spec we implement in task_06/task_08.

## Acceptance criteria

- [ ] A written spec (ours) of: provisioning steps, the chat endpoint + SSE event schema, and
      how the local Hermes server is launched — all traceable to fathah source files.
- [ ] A clear verdict: is the `:8642` SSE path stable enough to depend on, or do we need a
      thin adapter / wait for Hermes' stable API (#1925/#2491)?

## Primary sources

- `github.com/fathah/hermes-desktop` — README + source (`api/streaming`, installer/provision
  code, IPC). MIT.
- Cross-ref: `github.com/nesquena/hermes-webui` (alt pattern: stdlib HTTP on `:8787`, SSE,
  **direct module import** — explicitly "tightly coupled until stable API boundary work
  completes (#1925, #2491)") — a second data point on how others stream Hermes chat.
- `github.com/EKKOLearnAI/hermes-web-ui` / `outsourc-e/hermes-workspace` — additional
  reference wrappers if the above are insufficient.
