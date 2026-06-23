---
node_type: reference
title: Target architecture — thin Tauri wrapper over the full Hermes harness
service: desktop-app
status: active
updated: 2026-06-22
links:
  documents:
    - ../../app/src-tauri/src/lib.rs
    - ../../app/src/transport.ts
    - ../../app/src/App.tsx
    - ../../app/vite.config.ts
  relates_to:
    - ./hermes-backend.md
    - ../services/desktop-app/README.md
  depends_on:
    - ../decisions/0001-rust-host-trusted-loopback.md
    - ../decisions/0002-thin-wrapper-full-harness.md
---

# Target architecture

**Full harness, thin wrapper.** A native Tauri desktop shell gives a clean streaming chat;
the heavy power lives in the (already-provisioned) Hermes harness, which both Hermes and its
`ndcode` coding worker route through **one** proxy — the NeuralDeep hub.

```
┌─ Neural Deep.app  (Tauri v2 — native window) ───────────────────────────┐
│  WebView: React 19 chat  (app/src/App.tsx)                              │
│     │  app/src/transport.ts  →  isTauri ?                               │
│     │      ├─ Tauri:   invoke("chat_stream", Channel)  ── native path   │
│     │      └─ browser: fetch SSE via Vite /hermes proxy ── dev path     │
│     ▼                                                                    │
│  Rust host  (app/src-tauri/src/lib.rs)                                  │
│   • setup(): ensure_backend() — health-check :8642, spawn `hermes       │
│     gateway` if down (else reuse); kill on window Destroyed             │
│   • read_api_key() ← ~/.hermes/.env   (key never reaches the frontend)  │
│   • chat_stream(): reqwest POST /v1/chat/completions stream:true →      │
│     parse SSE → Channel.send(Delta|Done|Error)                         │
│      │  HTTP + SSE (loopback; Rust sends no Origin → CORS gate N/A)     │
│      ▼                                                                   │
│  FULL HARNESS: Hermes Agent v0.14.0 (~/.hermes, headless gateway)       │
│    api_server platform → OpenAI-compatible API on 127.0.0.1:8642        │
│        └─ skill "ndcode" ──spawns──▶ ndcode run '<task>'  (coding)      │
│  model.provider=custom · base_url + api_key  →                         │
└──────┼───────────────────────────────────────────────────────────────-─┘
       ▼
   NeuralDeep hub — https://api.neuraldeep.ru/v1  (free tier: qwen3.6-35b-a3b, gpt-oss-120b…)
```

## Components

| Layer | Where | Role |
|-------|-------|------|
| Chat UI | `app/src/App.tsx`, `styles.css` | thin streaming chat: engine badge, deltas, stop |
| Transport | `app/src/transport.ts` | auto-selects Tauri Channel vs browser fetch-SSE |
| Rust host | `app/src-tauri/src/lib.rs` | spawns/supervises Hermes; streams chat; holds the key |
| Backend | `~/.hermes` (Hermes v0.14.0) | full harness; OpenAI-compatible API on `:8642` |
| Coding worker | ndcode (`~/NeuralDeepCode`) | headless `run`/`serve`, pulled in via the `ndcode` skill |
| Proxy | NeuralDeep hub (external) | single OpenAI-compatible gateway for both harnesses |

## Two transports, one UI

- **Native (Tauri):** `isTauri === true` → `invoke("chat_stream", {messages, onEvent})` with a
  `Channel`. Rust is the trusted loopback client — no browser `Origin`, so Hermes' CORS gate
  is a non-issue, and `API_SERVER_KEY` never enters the webview.
- **Browser dev:** `isTauri === false` → fetch SSE through the Vite proxy `/hermes/*` →
  `:8642`, which injects `Authorization` and strips `Origin`. Same React UI.

Both transports drive Hermes' **session chat stream**
(`POST /api/sessions/{id}/chat/stream`), whose named SSE events carry not just content but
**reasoning** (`tool.progress`/`_thinking`) and **tool steps** — rendered as a collapsible
reasoning panel and tool chips. The app also surfaces the agent **working dir** (`agent_info`)
and the hub **tariff** (`subscription` → `hub.neuraldeep.ru/api/cli/status`).

See [hermes-backend](./hermes-backend.md) for the wire-level transport + SSE schema and
[verification](./verification.md) for the live end-to-end proof.

## Process lifecycle

```
Tauri Rust host (app process)
  ├─ ensure_backend(): reuse healthy :8642, else spawn `hermes gateway`   [lib.rs]
  │     └─ Hermes, via skill "ndcode", spawns: ndcode run '<task>'        [skills/ndcode]
  │            └─ ndcode → NeuralDeep hub
  └─ Hermes → NeuralDeep hub
WebView ⇄ Rust host ⇄ Hermes API  (invoke + Channel; loopback HTTP/SSE)
```
