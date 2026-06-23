---
node_type: reference
title: Hermes backend — transport, provisioning, single proxy
status: active
updated: 2026-06-22
links:
  documents: [../../app/vite.config.ts, ../../app/src/transport.ts]
  relates_to: [./architecture.md, ./verification.md, ../services/desktop-app/README.md]
---

# Hermes backend notes — transport, provisioning, single proxy

> Distilled spec of record for tasks 02/04/05/08. Traceable to the local install at
> `~/.hermes` and source at `~/.hermes/hermes-agent` (Hermes Agent v0.14.0, MIT).

## 1. What we run (task_02)

Hermes is already provisioned locally (git install method) at `~/.hermes`, with the
full source at `~/.hermes/hermes-agent` and a Python 3.11 venv at
`~/.hermes/hermes-agent/venv`. Launcher: `~/.local/bin/hermes`.

**Headless, account-less, local backend** — start command:

```bash
hermes gateway        # runs the messaging gateway; brings up the api_server platform
```

The chat transport is the **api_server gateway platform**
(`gateway/platforms/api_server.py`), enabled in `~/.hermes/config.yaml`:

```yaml
platforms:
  api_server:
    enabled: true
    extra: { port: 8642, host: 127.0.0.1 }
```

Health check: `GET http://127.0.0.1:8642/health` → `{"status":"ok","version":"0.14.0"}`.

No Nous Portal account is required (Q2 = **resolved: yes, fully local**). Models come
from a custom OpenAI-compatible provider (the NeuralDeep hub), configured in
`config.yaml` (see §3). No `hermes login` / portal credit tier is involved.

## 2. Chat transport (task_04 / Q1 / Q6)

`api_server.py` is itself an **OpenAI-compatible server** at `http://127.0.0.1:8642/v1`.
This is a first-class, documented server mode designed for external UIs (Open WebUI,
LobeChat, LibreChat…), **not** an internals-coupled hack → **Q6 = stable**.

Endpoints we use:

| Method | Route | Purpose |
|--------|-------|---------|
| `GET`  | `/health` | liveness |
| `GET`  | `/v1/models` | lists `hermes-agent` as the model |
| `POST` | `/v1/chat/completions` | OpenAI Chat Completions, `stream:true` → SSE |
| `GET`  | `/v1/capabilities` | machine-readable capabilities |
| `POST` | `/api/sessions` | create a session → `{session:{id}}` |
| `POST` | `/api/sessions/{id}/chat/stream` | **stateful chat with reasoning + tool steps** (named SSE — what the app uses) |
| session/run routes | `/api/sessions*`, `/v1/runs*` | SSE run events, **per-turn abort** via `POST /v1/runs/{id}/stop` (Q7 = yes) |

### Reasoning & tool steps (named SSE)

`/v1/chat/completions` **strips reasoning** (`_thinking` and tool progress are intentionally
not forwarded). To show reasoning, the app drives `POST /api/sessions/{id}/chat/stream`, which
emits **named** events (`event: <name>\ndata: <json>`):

| event | meaning | UI |
|-------|---------|----|
| `assistant.delta` | content token (`{delta}`) | main bubble |
| `reasoning.delta` | **model chain-of-thought** (`{delta}`) — requires the patch below | collapsible reasoning panel |
| `tool.progress` (`tool_name=="_thinking"`) | intermediate agent-turn reasoning (`{delta}`) | reasoning panel |
| `tool.started/completed/failed` | tool step | tool chips |
| `assistant.completed` | final content (fallback if no deltas streamed) | main bubble |
| `run.completed` | `{usage}` | token readout |

> **Reasoning patch.** Out of the box Hermes drops the model's reasoning from the session
> stream (it wires `stream_delta_callback`/`tool_progress_callback` but not
> `reasoning_callback`). `patches/hermes-api_server-reasoning.patch` threads a
> `reasoning_callback` through `_create_agent`/`_run_agent` and emits `reasoning.delta`.
> See [`patches/README.md`](../../patches/README.md). Re-apply after a Hermes upgrade.

### Subscription / tariff

`GET https://hub.neuraldeep.ru/api/cli/status` (bearer = hub key) returns
`{tier, user:{name,email}, limits:{rpm,parallel}, models:[{id,mode,ctx}]}` — surfaced as the
desktop's Plan badge (e.g. **Pro · kekslops · 8 models**). The Rust host calls it directly
with the hub key (never exposed to the webview). (The older LiteLLM `/key/info` on
`api.neuraldeep.ru` exposed spend/budget but no tier — superseded by `cli/status`.)

### SSE schema (verified live)

`POST /v1/chat/completions` with `stream:true` emits standard OpenAI chunks:

```
data: {"choices":[{"delta":{"role":"assistant"}}]}
data: {"choices":[{"delta":{"content":"NE"}}]}
data: {"choices":[{"delta":{"content":"URAL"}}]}
...
data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":15273,"completion_tokens":24,"total_tokens":15297}}
data: [DONE]
```

The ~15k prompt tokens confirm the **full harness** (skills + memory + tools) is engaged,
not a thin passthrough.

### Auth (task_02 T2.5)

`api_server` **requires** `API_SERVER_KEY` even on loopback (it refuses to start without
one — `api_server.py:4225`). We set it in `~/.hermes/.env`:

```
API_SERVER_KEY=nd-<random hex>
```

Clients send `Authorization: Bearer $API_SERVER_KEY`.

**CORS / Origin gate:** browser requests carry an `Origin` header; `_origin_allowed()`
returns 403 unless the origin is in `API_SERVER_CORS_ORIGINS` (`api_server.py:558,831`).
For the desktop dev proxy we **strip the `Origin` header** at the loopback proxy so Hermes
treats it as a non-browser caller (the proxy is the trusted client). Production Tauri uses a
custom-scheme webview / Rust host, same effect. (Alternative: set
`API_SERVER_CORS_ORIGINS=http://127.0.0.1:5173`.)

## 3. Single proxy: both harnesses → NeuralDeep hub (task_05 / Q5)

**Hub:** `https://api.neuraldeep.ru/v1`, key `sk-x9...` (free tier). One `curl`:

```bash
curl -s https://api.neuraldeep.ru/v1/chat/completions \
  -H "Authorization: Bearer $NEURALDEEP_API_KEY" -H "Content-Type: application/json" \
  -d '{"model":"qwen3.6-35b-a3b-noreason","messages":[{"role":"user","content":"hi"}]}'
```

Free-tier models observed: `gpt-oss-120b`, `qwen3.6-35b-a3b`(+`-noreason`),
`kimi-k2.6`, `gemma-4-31b`, `glm-5.2`, `deepseek-v4-pro`, embeddings/rerankers, `whisper-1`.

**Hermes wiring** (`~/.hermes/config.yaml`) — the exact config keys (Q5):

```yaml
model:
  default: qwen3.6-35b-a3b
  provider: custom
  base_url: https://api.neuraldeep.ru/v1
  api_key: sk-x9...
```

**ndcode wiring** — env (non-interactive, no `/login`, Q4):

```
NEURALDEEP_API_KEY=sk-x9...
NEURALDEEP_API_BASE=https://api.neuraldeep.ru/v1
```

Both harnesses hit the **same base URL + key** → one proxy, two clients (task_05 ✓).

## 4. End-to-end integration (task_08)

```
App (Vite dev :5173)  ──/hermes proxy (injects key, strips Origin)──▶  Hermes :8642
        ▲                                                                   │
        └────────────────── SSE token stream ◀──────────────────────────────┤
                                                                            ▼
                                              NeuralDeep hub (api.neuraldeep.ru/v1)
                                                   └─ Hermes, via skill "ndcode",
                                                      spawns `ndcode run '<task>'` ─▶ hub
```

Verified live through the running dev UI: prompt → Hermes agent loop → neuraldeep free
tier → streamed back, rendered as `NEURALDEEP_FREE_TIER_OK`
(see `../../neuraldeep-desktop-devrun.png`).
