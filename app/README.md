# Neural Deep Desktop — dev app

Thin chat wrapper over the **full Hermes harness**, routed through the **NeuralDeep hub**.
Every model call: `App → Hermes (:8642) → api.neuraldeep.ru/v1`.

## Prerequisites

1. **Hermes backend** provisioned at `~/.hermes` and pointed at the hub
   (`~/.hermes/config.yaml` → `model.provider: custom`, `base_url: https://api.neuraldeep.ru/v1`).
2. **`API_SERVER_KEY`** set in `~/.hermes/.env` (the api_server refuses to start without it).
3. Start the backend:
   ```bash
   hermes gateway
   curl http://127.0.0.1:8642/health    # → {"status":"ok",...}
   ```

## Run — native Tauri desktop app (recommended)

```bash
cd app
bun install
bun run tauri:dev      # builds the Rust host + opens the native "Neural Deep" window
```

The Rust host (`src-tauri/`) **spawns & supervises** the Hermes gateway (reusing it if it's
already healthy on :8642) and **streams chat itself** from Hermes into the webview over a
Tauri `Channel`. Because Rust is the trusted loopback client, there is no CORS/Origin gate
and the API key never reaches the frontend.

Produce a distributable bundle (signed `.app` + `.dmg` on macOS):

```bash
bun run tauri:build
```

> Notarization needs the `notarytool` keychain-profile name (open item Q8 in
> `../docs/verification.md`); signing reuses Developer ID `Valeriy Kovalsky (A933C2TJXU)`.

## Run — browser dev (no native shell)

```bash
bun run dev            # Vite dev server on http://127.0.0.1:5173
```

In a plain browser tab the app detects it is **not** under Tauri and falls back to the Vite
proxy: `/hermes/*` → `http://127.0.0.1:8642/*`, injecting `Authorization: Bearer
$API_SERVER_KEY` (from `~/.hermes/.env`) and stripping the browser `Origin` so Hermes' CORS
gate accepts it. Override the backend with `HERMES_URL=...`.

## Transport selection

`src/transport.ts` picks the path automatically: **Tauri → Rust `chat_stream` Channel**;
**browser → fetch SSE through the Vite proxy**. Same React UI either way.

## What's thin vs. full

- **Thin (this app):** chat input, SSE streaming render, engine/health badge, stop button.
- **Full (underneath):** Hermes memory, skills (incl. the `ndcode` coding-delegation skill),
  planning, tool use — all reachable, none reimplemented here.
