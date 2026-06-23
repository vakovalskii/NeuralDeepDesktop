---
node_type: reference
title: KB ontology — vocabularies for this repo
status: active
updated: 2026-06-22
links:
  relates_to: [./README.md]
---

# KB ontology (OntoShip) — this repo

md + git is the source of truth; the index/graph are derived. Every doc under `docs/` has a
`node_type`, frontmatter properties, and ≥1 typed link.

## node_type vocabulary

`service · reference · runbook · gotcha · decision · plan · guide · report · index`

## status vocabulary

`active · draft · deprecated · archived`

## service vocabulary (this repo)

| service | meaning |
|---------|---------|
| `desktop-app` | the Tauri shell — React UI, transport, Rust host (`app/`) |
| `backend` | Hermes harness + its OpenAI-compatible API on `:8642` (`~/.hermes`) |
| `build` | packaging, signing, licensing |
| `meta` | KB about the KB |

## link types

`documents` / `implemented_by` (doc ↔ code) · `depends_on` / `relates_to` (doc ↔ doc) ·
`supersedes` (replacement ↔ deprecated).

## folders

`docs/reference/` specs · `docs/services/<svc>/` per-component · `docs/guides/` how-to ·
`docs/decisions/` ADRs · each folder carries a `README.md` index.
