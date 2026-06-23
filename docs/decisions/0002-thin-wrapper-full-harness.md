---
node_type: decision
title: Thin wrapper, full harness — reuse Hermes + ndcode, one proxy
service: desktop-app
status: active
updated: 2026-06-22
links:
  relates_to: [../reference/architecture.md, ../reference/packaging.md]
  documents: [../../skills/ndcode/SKILL.md]
---

# Decision: thin wrapper, full harness

## Context

The product north star is to ship the *complete* power of Hermes + ndcode behind a very light
desktop, not a stripped toy.

## Decision

- **Hermes is the brain**, run headless and unmodified (adopt upstream, MIT). The wrapper
  spawns it; it does not reimplement the control panel.
- **ndcode is a tool Hermes pulls in** via the `ndcode` skill — not a peer the user toggles.
- **One proxy:** both Hermes (`config.yaml` custom provider) and ndcode (`NEURALDEEP_*` env)
  point at the same NeuralDeep hub base URL + key.
- **The wrapper is thin:** Tauri shell = streaming chat + minimal settings + backend
  lifecycle. The weight lives in the (provisioned) harness, not the installer.
- **Dev shortcut:** reuse the already-installed `~/.hermes` instead of first-run provisioning;
  the `uv` + standalone-CPython path (see [packaging](../reference/packaging.md)) is the
  ship-time story.

## Consequences

- Minimal installer; full agent capability (memory, skills, planning, tool use).
- Transport fragility is isolated behind one adapter
  ([transport.ts](../../app/src/transport.ts) / the Rust host); Hermes version is pinned.
- Whole stack is MIT/Apache/PSF → sellable as Neural Deep (see
  [licensing](../reference/licensing.md)).
