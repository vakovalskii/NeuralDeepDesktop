# task_03 — ndcode coding worker: headless mode + Hermes wiring

> Status: `research` + `build` · Depends on: task_01, task_02 · Blocks: task_08

## Goal

Make Hermes able to *pull in ndcode* as its coding worker. The blocker to resolve: ndcode
must be invokable **headless** (one task in, result out) the way Hermes' skill expects.

## What ndcode is (research findings)

- **NeuralDeepCode (ndcode)** = "a rebranded, hub-integrated **fork of `sst/opencode`**",
  MIT (retains upstream copyright), **TypeScript 96%, Bun ≥1.3**. (Source: repo README.)
- Install/run today: `git clone … && bun install`, launch `bun run dev`.
- Hub integration baked in: slash commands `/login` (browser SSO → configures the
  `neuraldeep` provider) and `/status` (tier/budget/limits/models). Models e.g.
  `qwen3.6-35b-a3b` (256k), `gpt-oss-120b` (131k). Config via `NDC_*` env;
  endpoint overrides `NEURALDEEP_HUB`, `NEURALDEEP_API_BASE`.
- **This means the "single proxy" is already solved for the coding hand** — ndcode natively
  points at the NeuralDeep hub.

## The blocker

- ndcode's README documents **only an interactive TUI** (`bun run dev`). **No headless /
  one-shot mode is documented.**
- But Hermes' coding-agent skill drives the worker **non-interactively**:
  `opencode run '<task>'` (one-shot) and interactive `pty` sessions, polling stdout via
  `process(poll|log|submit)`. A pure TUI can't be driven autonomously.
- **However ndcode is a fork of opencode, which HAS `opencode run` and `opencode serve`**
  (HTTP/SSE). So the headless capability almost certainly **exists in ndcode's code**, just
  un-/under-documented or disabled by the rebrand. This must be verified, not assumed.

## Tasks

- [ ] **T3.1** Clone ndcode and grep the source for the upstream opencode subcommands:
      `run`, `serve`, the CLI command registry. Confirm whether `ndcode run '<task>'` and/or
      `ndcode serve` still work. (Upstream entry points: opencode's CLI + `opencode serve`
      → HTTP on `127.0.0.1:4096`, OpenAPI at `/doc`, SSE at `/event`; SDK `@opencode-ai/sdk`
      `createOpencodeServer`/`createOpencodeClient`.)
- [ ] **T3.2** If headless is present → document the exact command/flags + how to point it at
      the NeuralDeep hub non-interactively (env, since `/login` is interactive: likely
      `NEURALDEEP_API_BASE` + an API key env instead of SSO).
- [ ] **T3.3** If headless is missing/broken → restore it from upstream opencode (it's an
      MIT fork tracking upstream; re-enabling `run`/`serve` should be a small change). Pin a
      ndcode commit/build for the bundle.
- [ ] **T3.4** Decide the invocation contract Hermes uses: **`ndcode run '<task>'`** (simple,
      matches the existing skill) vs **`ndcode serve` + HTTP/SSE** (richer, streamable). Lean
      to `run` first (least change to Hermes skill), keep `serve` as an upgrade.
- [ ] **T3.5** Author the Hermes skill **`ndcode`**: copy
      `skills/autonomous-ai-agents/opencode/SKILL.md`, swap `opencode` → `ndcode`, set the
      provider to `neuraldeep`, point at the hub. Drop it into `~/.hermes/skills/`.
- [ ] **T3.6** Ensure the bundled `ndcode` is on `PATH` for the Hermes process (so the skill's
      `ndcode …` resolves). Cross-ref task_06 (bundling) and task_08 (process env).

## Build artifact for the bundle

ndcode runs on **Bun**, not Node — packaging options (decide in task_06):
- `bun build --compile` → a **single self-contained native executable** (ideal: one binary
  per Mac/Win target, like opencode's own binaries). Verify ndcode compiles this way.
- Fallback: bundle a `bun` runtime + ndcode dist.

## Acceptance criteria

- [ ] A reproducible **headless** invocation: `ndcode run '<task>'` (or `serve`) that uses the
      NeuralDeep hub without interactive `/login`.
- [ ] A single native `ndcode` binary per target (preferred) or a documented run command.
- [ ] A working Hermes `ndcode` skill that successfully delegates a coding task end-to-end.

## Primary sources

- `github.com/vakovalskii/NeuralDeepCode` — README (fork of sst/opencode, MIT, Bun, `NDC_*`,
  `NEURALDEEP_HUB`, `NEURALDEEP_API_BASE`).
- `github.com/sst/opencode`, `opencode.ai/docs/cli`, `opencode.ai/docs/server` — upstream
  `run`/`serve`, ports, SSE, OpenAPI.
- `github.com/NousResearch/hermes-agent/blob/main/skills/autonomous-ai-agents/opencode/SKILL.md`
  — the orchestration pattern to copy.
