# Patches

Local patches applied to the **provisioned Hermes harness** (`~/.hermes/hermes-agent`),
which lives outside this repo and is overwritten on a Hermes upgrade. Re-apply after any
`hermes update`.

## `hermes-api_server-reasoning.patch`

**Why:** Hermes' `api_server` session-chat stream did not forward the model's
chain-of-thought. The plumbing exists (`agent.reasoning_callback` →
`run_agent._fire_reasoning_delta`, fed by streaming `reasoning_content` deltas), but
`_create_agent` / `_run_agent` never wired a `reasoning_callback`, so reasoning was generated
(and paid for) but dropped. Only sporadic intermediate-turn `reasoning.available`
(`tool.progress`/`_thinking`) events leaked through.

**What it does:** threads a `reasoning_callback` through `_create_agent` and `_run_agent`,
and in `_handle_session_chat_stream` emits a new named SSE event
`event: reasoning.delta\ndata: {"delta": "<text>"}` for each streaming reasoning chunk. The
desktop renders these in the collapsible **reasoning** panel.

**Apply:**

```bash
cd ~/.hermes/hermes-agent
patch -p0 < /path/to/NeuralDeepDesktop/patches/hermes-api_server-reasoning.patch
pkill -f "hermes gateway"; hermes gateway   # restart to load
```

A timestamped backup of the original is at
`~/.hermes/hermes-agent/gateway/platforms/api_server.py.bak.ndd`.

> Upstream-friendly: this is additive (new optional kwarg + new event), so it could be sent
> as a PR to NousResearch/hermes-agent to make reasoning a first-class session-stream event.
