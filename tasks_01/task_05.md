# task_05 — Single model proxy (NeuralDeep hub / LiteLLM)

> Status: `research` + `build` · Depends on: task_01 · Blocks: task_02, task_03, task_08

## Goal

Make **both** harnesses (Hermes core + ndcode worker) route **every** model call through
**one** OpenAI-compatible gateway — the NeuralDeep hub — so models, keys, routing, and cost
are centralized. The proxy stays **external** to the desktop bundle (lightest packaging).

## Decision

- **Proxy = external NeuralDeep hub** (OpenAI-compatible: one base URL + key). The desktop
  does **not** bundle a proxy. Rationale: zero packaging weight, single source of truth for
  models/keys/cost, matches how ndcode already works.
- **LiteLLM** is the documented fallback/alt if a *local* proxy is ever wanted (it's an
  OpenAI-compatible proxy with model list, virtual keys, routing, cost/observability) — keep
  as an optional, not in the default bundle.

## Per-harness wiring

### ndcode (already solved)
- ndcode natively targets the hub: `/login` SSO, provider `neuraldeep`, endpoint overrides
  `NEURALDEEP_HUB` / `NEURALDEEP_API_BASE`, config via `NDC_*` env. (Source: ndcode README.)
- For **headless** use (task_03) we need a **non-interactive** auth path (API key env instead
  of browser `/login`). Confirm the env var ndcode reads for a hub key.

### Hermes (to wire)
- Hermes supports "custom endpoints" + provider switching (`hermes model`, and
  `GET/PUT /api/config`, `/api/config/raw` YAML, `/api/model/options|set`, `/api/env`).
- **Task:** confirm the exact config keys to register an **OpenAI-compatible custom provider**
  pointing at the hub base URL + key, and to select a hub model as the main model (plus
  auxiliary models for titles/summaries). Likely a `providers`/`model` block in the Hermes
  config YAML and/or an env-based OpenAI-compatible provider.

## Tasks

- [ ] **T5.1** Confirm the NeuralDeep hub's OpenAI-compatible surface: base URL shape
      (`…/v1`?), `/models` listing, `/chat/completions` streaming, auth header. Capture one
      working `curl`.
- [ ] **T5.2** Hermes: write the config (YAML via `/api/config/raw` or file) that registers
      the hub as a custom OpenAI-compatible provider and sets main + auxiliary models. Verify
      via `/api/model/info`.
- [ ] **T5.3** ndcode: confirm the non-interactive hub auth (key env) for headless runs;
      document the env set the sidecar injects.
- [ ] **T5.4** Verify **both** harnesses hit the **same** base URL/key (grep their effective
      config at runtime). One proxy, two clients.
- [ ] **T5.5** (Optional) Document a local-LiteLLM profile for offline/dev: minimal
      `config.yaml` (model_list → hub or local models, virtual key), and how each harness
      points at `http://127.0.0.1:4000`.

## Acceptance criteria

- [ ] One base URL + key drives Hermes *and* ndcode; switching the hub URL/key in settings
      reconfigures both.
- [ ] A model picker in the wrapper is populated from the hub (via Hermes `/api/model/options`
      and/or the hub `/models`).
- [ ] No second proxy is bundled by default.

## Primary sources

- `github.com/vakovalskii/NeuralDeepCode` — `NEURALDEEP_HUB`, `NEURALDEEP_API_BASE`, `NDC_*`,
  `neuraldeep` provider.
- `github.com/NousResearch/hermes-agent` — `web/src/lib/api.ts` config/model/env endpoints;
  `docs/` provider configuration.
- LiteLLM proxy docs (`docs.litellm.ai`) — OpenAI-compatible proxy, model_list, virtual keys,
  routing, cost (optional local path).
