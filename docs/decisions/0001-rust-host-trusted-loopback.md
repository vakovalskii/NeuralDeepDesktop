---
node_type: decision
title: Rust host is the trusted loopback client (chat over a Channel)
service: desktop-app
status: active
updated: 2026-06-22
links:
  documents: [../../app/src-tauri/src/lib.rs, ../../app/src/transport.ts]
  relates_to: [../reference/architecture.md, ../reference/hermes-backend.md]
---

# Decision: Rust host is the trusted loopback client

## Context

Hermes' `api_server` (`:8642`) requires `API_SERVER_KEY` and, for **browser** callers,
rejects any request whose `Origin` is not in `API_SERVER_CORS_ORIGINS` (403). A naive webview
calling `:8642` directly would 403, and putting the key in the frontend is undesirable.

## Decision

In the Tauri app, **the Rust host makes the HTTP/SSE call**, not the webview. A Tauri command
`chat_stream(messages, Channel)` opens `POST /v1/chat/completions` with the bearer key
(read from `~/.hermes/.env`) and streams parsed deltas back to the UI over a `Channel`.

## Consequences

- Rust sends **no browser `Origin`** → Hermes treats it as a non-browser client → no CORS gate.
- `API_SERVER_KEY` **never enters the webview**.
- The browser-dev fallback keeps the same property via the Vite proxy (injects key, strips
  `Origin`) — see [transport.ts](../../app/src/transport.ts).
- Per-turn abort and richer run events remain available on `:8642` if needed later (Q7).
