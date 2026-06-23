---
node_type: service
title: desktop-app — Neural Deep Tauri shell
service: desktop-app
status: active
updated: 2026-06-22
links:
  documents:
    - ../../../app/src-tauri/src/lib.rs
    - ../../../app/src-tauri/tauri.conf.json
    - ../../../app/src/App.tsx
    - ../../../app/src/transport.ts
  relates_to:
    - ../../reference/architecture.md
    - ../../reference/hermes-backend.md
  depends_on:
    - ../../guides/run-dev.md
---

# desktop-app — Neural Deep (Tauri shell)

The native desktop application: a thin React 19 chat in a Tauri v2 webview, driven by a Rust
host that owns the Hermes backend lifecycle and the chat stream.

## Layout

```
app/
├── index.html · vite.config.ts        # Vite entry + dev /hermes proxy
├── package.json                       # react19 + @tauri-apps/{api,cli}
├── src/
│   ├── main.tsx                       # React bootstrap
│   ├── App.tsx                        # chat UI (engine badge, streaming, stop)
│   ├── transport.ts                   # Tauri Channel | browser fetch-SSE selector
│   └── styles.css
└── src-tauri/
    ├── Cargo.toml                     # tauri2, reqwest(stream), tokio, futures-util, dirs
    ├── tauri.conf.json                # productName, Developer ID, icons, identifier
    ├── capabilities/default.json
    └── src/{main.rs, lib.rs}          # Rust host
```

## Rust host (`src-tauri/src/lib.rs`)

| Item | Responsibility |
|------|----------------|
| `read_env_key()` / `read_hub_key()` | `API_SERVER_KEY` from `~/.hermes/.env`; hub key from `.env`/`config.yaml` |
| `ensure_backend()` | health-check `:8642`; spawn `hermes gateway` (cwd = home) if down, else reuse |
| `hermes_health()` (command) | proxied health for the engine badge |
| `create_session()` (command) | `POST /api/sessions` → session id |
| `chat_stream(session_id, message, Channel)` (command) | drive `POST /api/sessions/{id}/chat/stream`; parse **named** SSE; emit `Delta`/`Reasoning`/`Tool`/`Done`/`Error` |
| `agent_info()` (command) | the agent working dir (shown as `📁 ~`) |
| `subscription()` (command) | hub tariff via `hub.neuraldeep.ru/api/cli/status` (tier / user / models) |
| window `Destroyed` handler | kill the gateway **only if we spawned it** |

## Frontend (`src/`)

`App.tsx` renders the chat (streamed content, a collapsible **reasoning** panel fed by
`tool.progress`/`_thinking`, tool-step chips, token usage), a **subscription** badge, an
engine badge, and a status bar with the **working folder** + a reasoning toggle.
`transport.ts` picks the path by `isTauri` (`__TAURI_INTERNALS__`): native → Rust commands +
`Channel`; browser → the same Hermes session API through the Vite `/hermes` proxy. The
reasoning/tool events come from Hermes' session stream, **not** `/v1/chat/completions` (which
strips reasoning).

## Run

See [run-dev](../../guides/run-dev.md). TL;DR: `bun run tauri:dev` (native) or `bun run dev`
(browser). Distributable: `bun run tauri:build` (signing/notarization → see
[packaging](../../reference/packaging.md)).
