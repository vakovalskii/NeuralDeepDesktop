---
node_type: guide
title: Run the app in dev
service: desktop-app
status: active
updated: 2026-06-22
links:
  documents: [../../app/package.json, ../../app/vite.config.ts]
  relates_to: [../services/desktop-app/README.md, ../reference/architecture.md]
---

# Run the app in dev

## Prerequisites

1. Hermes provisioned at `~/.hermes`, pointed at the hub (`~/.hermes/config.yaml` →
   `model.provider: custom`, `base_url: https://api.neuraldeep.ru/v1`).
2. `API_SERVER_KEY` set in `~/.hermes/.env` (the api_server refuses to start without it).
3. Toolchain: `bun`, Rust (`cargo`), Xcode CLT (macOS).

The Rust host starts Hermes automatically; you can also start it manually:

```bash
hermes gateway
curl http://127.0.0.1:8642/health     # → {"status":"ok",...}
```

## Native Tauri app (recommended)

```bash
cd app && bun install
bun run tauri:dev          # builds the Rust host, opens the native "Neural Deep" window
```

The Rust host spawns/supervises Hermes (reuses a healthy `:8642`) and streams chat over a
`Channel`. No CORS/Origin gate; the API key stays in Rust.

## Browser dev (no native shell)

```bash
cd app && bun run dev      # Vite on http://127.0.0.1:5173
```

Falls back to the Vite `/hermes` proxy (injects the key, strips `Origin`). Override the
backend with `HERMES_URL=...`.

## Verify

Open the app, confirm the engine badge reads **"Hermes vX online → NeuralDeep hub"**, click
**Test the free tier** → expect a streamed `NEURALDEEP_FREE_TIER_OK`.

## Build a distributable

```bash
bun run tauri:build        # signed .app + .dmg (macOS)
```

Notarization needs the notarytool profile — see [packaging](../reference/packaging.md) and
open question Q8 in [verification](../reference/verification.md).
