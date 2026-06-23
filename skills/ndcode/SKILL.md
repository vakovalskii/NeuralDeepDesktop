---
name: ndcode
description: "Delegate coding to ndcode (NeuralDeepCode) CLI — MIT fork of opencode, routed through the NeuralDeep hub."
version: 1.0.0
author: Neural Deep
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Coding-Agent, ndcode, NeuralDeepCode, Autonomous, Refactoring, Code-Review]
    related_skills: [opencode, claude-code, codex, hermes-agent]
---

# ndcode (NeuralDeepCode) CLI

Use **ndcode** as Hermes' autonomous coding worker, orchestrated via the terminal/process
tools. ndcode is a rebranded, hub-integrated MIT fork of `sst/opencode` (TypeScript, Bun).
It retains upstream opencode's headless `run` and `serve` subcommands (verified in
`packages/ndcode/src/index.ts`: `.command(RunCommand)`, `.command(ServeCommand)`).

Every model call ndcode makes is routed through the **NeuralDeep hub**
(`https://api.neuraldeep.ru/v1`) — the same single proxy Hermes uses.

## When to Use

- You want an external coding agent to implement/refactor/review code
- Long-running coding sessions with progress checks
- Parallel task execution in isolated workdirs/worktrees

## Non-interactive hub auth (no browser /login)

ndcode's `/login` is an interactive browser SSO. For headless delegation, **skip it** and
authenticate via env instead:

```
NEURALDEEP_API_KEY=sk-...           # hub key (free tier ok)
NEURALDEEP_API_BASE=https://api.neuraldeep.ru/v1
```

These are already exported for the Hermes process by the Neural Deep desktop host, so
`ndcode run` works without any interactive step.

## Binary Resolution

Production bundle ships a single native `ndcode` binary (`bun build --compile`) on PATH.
In dev, a launcher wrapper execs the Bun entrypoint. Verify:

```
terminal(command="which -a ndcode")
terminal(command="ndcode --version")
```

## One-Shot Tasks (preferred)

Use `ndcode run` for bounded, non-interactive tasks:

```
terminal(command="ndcode run 'Add retry logic to API calls and update tests'", workdir="~/project")
```

Force a hub model:

```
terminal(command="ndcode run 'Refactor auth module' --model neuraldeep/qwen3.6-35b-a3b", workdir="~/project")
```

Attach context files with `-f`, show thinking with `--thinking`, machine output with
`--format json` (same flag surface as upstream opencode).

## Interactive Sessions (Background)

For iterative work, start the TUI in background and drive it via `process`:

```
terminal(command="ndcode", workdir="~/project", background=true, pty=true)   # returns session_id
process(action="submit", session_id="<id>", data="Implement OAuth refresh flow and add tests")
process(action="poll", session_id="<id>")
process(action="log", session_id="<id>")
process(action="kill", session_id="<id>")   # exit with Ctrl+C (\x03) or kill — never /exit
```

## Server Mode (richer, streamable)

`ndcode serve` starts an HTTP/SSE server (upstream opencode default `127.0.0.1:4096`,
OpenAPI at `/doc`, SSE at `/event`). Use when you want to stream events programmatically;
otherwise prefer `run`.

## Verification

```
terminal(command="ndcode run 'Respond with exactly: NDCODE_SMOKE_OK'")
```

Success: output includes `NDCODE_SMOKE_OK`, exits without provider/model errors, and the
call appears against the NeuralDeep hub.

## Rules

1. Prefer `ndcode run` for one-shot automation — no pty needed.
2. Authenticate via `NEURALDEEP_API_KEY` env, never the interactive `/login`, in headless mode.
3. Scope each ndcode session to a single repo/workdir; use separate workdirs for parallel work.
4. For long tasks, stream progress from `process(action="log"|"poll")`.
5. Report concrete outcomes (files changed, tests, remaining risks).
6. Exit interactive sessions with Ctrl+C or kill, never `/exit`.
