# task_10 — Licensing & commercialization

> Status: `research` (mostly done) · Depends on: task_01 · Blocks: ship

## Goal

Confirm we can legally **rebrand and sell** the whole stack, and list the conditions.

## License inventory (verified where noted)

| Component | License | Sell/rebrand? | Condition |
|-----------|---------|---------------|-----------|
| **Hermes Agent** (NousResearch/hermes-agent) | **MIT** (verified: LICENSE © 2025 Nous Research; `package.json` license: MIT, `private:true` is just an npm flag) | ✅ yes | keep MIT notice + copyright in copies |
| **ndcode** (vakovalskii/NeuralDeepCode) | **MIT** (fork of sst/opencode, retains upstream copyright) | ✅ yes (it's yours) | keep upstream opencode MIT notice |
| **opencode** (sst/opencode) | MIT | ✅ | notice |
| **fathah/hermes-desktop** (patterns/backend we lift) | **MIT** | ✅ | notice if we copy code |
| **LiteLLM** (optional) | MIT | ✅ | notice |
| **uv / python-build-standalone** | Apache-2.0 / PSF (permissive) | ✅ | notices |
| **Tauri** | MIT/Apache-2.0 | ✅ | notice |
| Our Tauri shell (from NeuralDeskApp) | ours | ✅ | — |

→ **The whole chain is permissive (MIT/Apache/PSF) → sellable under our own brand.**

## Conditions / gotchas (important for "sell")

1. **Trademark ≠ code.** MIT licenses the *code*, not the names/logos "Hermes", "Hermes
   Agent", "Nous Research", "opencode". Ship under **our brand (Neural Deep)**; do **not**
   market as "Hermes" or imply Nous endorsement. Acceptable: "powered by Hermes Agent (MIT)"
   in credits.
2. **Preserve notices.** Bundle a `THIRD_PARTY_LICENSES` / NOTICE file with the MIT texts +
   copyrights for Hermes, opencode/ndcode, fathah (if code lifted), LiteLLM, uv, Tauri.
3. **Models ≠ harness.** These licenses cover the *harness code*. If we ever ship *model
   weights* (e.g. Nous Hermes models), those have their **own** licenses — out of scope here
   (models come via our proxy, not bundled).
4. **The official Hermes Desktop binary is NOT ours to rebrand** — it's a closed prebuilt,
   gated by Nous Portal credit tiers (Free/Plus/Super/Ultra). We build our own from the MIT
   sources instead. (Confirmed: no desktop app source in the MIT repo.)
5. **UI we adopt must also be permissive.** If we ever reuse a general chat UI, check it:
   **LibreChat** = MIT ✅; **Open WebUI** has a **branding-retention clause** in recent
   versions (not pure MIT) ⚠️ — avoid for a sellable rebrand unless terms are met. Our plan
   builds the UI (task_07), so this is only a caution.
6. **fathah is "not affiliated with Nous"** and community-maintained — fine to lift under MIT;
   just attribute if we copy code.

## Tasks

- [ ] **T10.1** Generate a `THIRD_PARTY_LICENSES.md` aggregating all MIT/Apache/PSF notices.
- [ ] **T10.2** Add a credits/about line: "Powered by Hermes Agent and opencode (MIT)".
- [ ] **T10.3** Trademark pass: ensure no use of others' marks/logos in branding, store
      listings, or icon.
- [ ] **T10.4** If any third-party UI code is copied (fathah patterns), keep its notice.
- [ ] **T10.5** Confirm the NeuralDeep hub's own ToS permits reselling access via our app (it's
      yours — just record it).

## Acceptance criteria

- [ ] A complete, accurate `THIRD_PARTY_LICENSES.md` in the bundle.
- [ ] Branding uses only our marks; attributions present where required.
- [ ] Documented green-light: "we may sell this under Neural Deep."

## Primary sources

- Hermes `LICENSE` (MIT © 2025 Nous Research), `package.json` (license MIT, private flag).
- ndcode README (MIT, fork of sst/opencode). sst/opencode LICENSE (MIT).
- fathah/hermes-desktop (MIT). LiteLLM (MIT). uv (Apache-2.0). Tauri (MIT/Apache-2.0).
- Open WebUI license terms (branding clause) — verify before any reuse.
