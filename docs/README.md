---
node_type: index
title: Neural Deep Desktop — knowledge base
status: active
updated: 2026-06-22
links:
  relates_to: [./ontology.md, ./reference/architecture.md, ./services/desktop-app/README.md]
---

# Neural Deep Desktop — knowledge base

Thin Tauri desktop wrapper over the **full Hermes harness**, routed through the **NeuralDeep
hub**. Start at [architecture](./reference/architecture.md).

## Map

- **Architecture & specs** → [`reference/`](./reference/README.md)
  - [architecture](./reference/architecture.md) — target shape, two transports, lifecycle
  - [hermes-backend](./reference/hermes-backend.md) — `:8642` transport, SSE schema, single proxy
  - [packaging](./reference/packaging.md) — uv provisioning, signing, notarization
  - [verification](./reference/verification.md) — Q1–Q10 + end-to-end gate
  - [licensing](./reference/licensing.md) — MIT/Apache/PSF, sellable as Neural Deep
- **Components** → [`services/`](./services/README.md)
  - [desktop-app](./services/desktop-app/README.md) — Tauri shell (UI + transport + Rust host)
- **How-to** → [`guides/`](./guides/README.md)
  - [run-dev](./guides/run-dev.md)
- **Decisions** → [`decisions/`](./decisions/README.md)
  - [0001 — Rust host is the trusted loopback client](./decisions/0001-rust-host-trusted-loopback.md)
  - [0002 — Thin wrapper, full harness](./decisions/0002-thin-wrapper-full-harness.md)
- **Ontology** → [ontology](./ontology.md)

## Original plan

The research + task breakdown lives in [`../tasks_01/`](../tasks_01/) (task_01…task_11);
resolution status is consolidated in [verification](./reference/verification.md).
