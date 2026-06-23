# Neural Deep Desktop — entry point

Thin **Tauri** desktop wrapper over the **full Hermes harness**, with every model call routed
through the **NeuralDeep hub** (one OpenAI-compatible proxy). The app is built and runs
natively; chat streams UI → Rust host → Hermes `:8642` → neuraldeep hub → back.

## Where things are

- **App code:** `app/` — React UI (`src/App.tsx`), transport selector (`src/transport.ts`),
  Rust host (`src-tauri/src/lib.rs`).
- **Knowledge base:** [`docs/`](docs/README.md) — start at
  [architecture](docs/reference/architecture.md). Ontology: [docs/ontology.md](docs/ontology.md).
- **Coding-worker skill:** [`skills/ndcode/SKILL.md`](skills/ndcode/SKILL.md).
- **Licenses:** [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md).

## Run

```bash
cd app && bun install && bun run tauri:dev    # native window  (or: bun run dev → browser)
```

See [docs/guides/run-dev.md](docs/guides/run-dev.md).

## Docs convention

This repo uses the **OntoShip / GitMark** ontology: every doc under `docs/` has a `node_type`,
frontmatter, and typed links; each folder has a `README.md` index. When adding/editing docs,
follow the `kb-curate` rules and re-run `gitmark lint && gitmark index`.
