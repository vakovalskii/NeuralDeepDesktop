# task_08 — Integration & transport (HTTP/SSE, process lifecycle)

> Status: `build` · Depends on: task_02, task_03, task_04, task_06 · Blocks: task_07

## Goal

Define and implement the wiring that makes the pieces one app: who spawns what, on which
ports, and the exact UI⇄Hermes chat transport + Hermes⇄ndcode delegation.

## Process lifecycle (what spawns what)

```
Tauri Rust host (app process)
  ├─ spawn (first run): uv → provision full Hermes into app-data        [task_06]
  ├─ spawn (each launch): Hermes headless API server (local port)        [task_02/04]
  │     └─ Hermes, via skill "ndcode", spawns: ndcode run '<task>'       [task_03]
  │            └─ ndcode → NeuralDeep hub (models)                        [task_05]
  └─ Hermes → NeuralDeep hub (models)                                    [task_05]
UI (WebView) ⇄ Rust host ⇄ Hermes API  (HTTP + SSE)
```

- **Single supervised child from Rust's view:** the Hermes server. ndcode is a *grandchild*
  Hermes spawns itself (its native skill pattern) — Rust just ensures `ndcode` is on Hermes'
  `PATH` and points at the hub.
- Ports: bind Hermes to **loopback only**, on a chosen/auto port (avoid clashes — official
  `web/` uses `:9119`, fathah uses `:8642`; pick one and make it configurable). ndcode, if run
  as `serve`, uses its own loopback port; if `run`, no port.

## Transport: UI ⇄ Hermes

- **Resolve the chat route first (task_04 dependency).** Two candidates:
  - fathah-style: POST a message to the local Hermes API, read **SSE** token/tool stream
    (the `:8642` path). Preferred if stable.
  - official `:9119` + a WebSocket (`buildWsUrl`) if that's where chat actually lives.
- **Translate to the wrapper's render model:** map Hermes' stream events
  (token delta, tool start/progress/result — including the ndcode delegation steps, usage,
  done) to the React chat's message/streaming shapes (reuse NeuralDeskApp's `StreamMessage`
  rendering where it fits).
- **Auth:** loopback session-token injection or `--insecure` loopback (task_02). The desktop
  is the only client; keep it local and simple.

## Transport: Hermes ⇄ ndcode

- Via the **`ndcode` skill** (task_03): Hermes runs `ndcode run '<task>'` (or `serve`),
  polls/reads output, folds results back into the conversation. We do **not** build a separate
  ndcode bridge in the wrapper — Hermes owns this. The wrapper only *renders* the delegation
  as tool steps.

## Tasks

- [ ] **T8.1** Lock the Hermes chat endpoint + streaming event schema (from task_04). Write it
      down as the integration contract.
- [ ] **T8.2** Implement the Rust host: spawn/supervise Hermes (env: hub URL/key, ndcode on
      PATH, app-data dir), health-check, restart, shutdown.
- [ ] **T8.3** Implement the UI Hermes client: send message, consume SSE, render
      tokens/tool-steps/usage; stop = abort the Hermes turn.
- [ ] **T8.4** Verify end-to-end: user prompt → Hermes reasons → delegates a coding task →
      ndcode runs against the hub → results stream back into chat.
- [ ] **T8.5** Port selection + collision handling; make Hermes (and ndcode `serve`) ports
      configurable; loopback-only binding.
- [ ] **T8.6** Error surfaces: backend down, provisioning incomplete, hub auth failure, ndcode
      missing — all shown cleanly in the thin UI.

## Acceptance criteria

- [ ] A single user message can travel UI → Hermes → ndcode → hub → back, streamed, with tool
      steps visible.
- [ ] Clean lifecycle: launch provisions/starts backend; quit stops it; crash restarts it.
- [ ] All inter-process traffic is loopback; auth handled.

## Open questions (→ task_11)

- Exact chat route/port + SSE schema (blocked on task_04 source read).
- Does Hermes expose per-turn abort over the local API?
- Streaming granularity of ndcode delegation steps as seen by Hermes' stream.
