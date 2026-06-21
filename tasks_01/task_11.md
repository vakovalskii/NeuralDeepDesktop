# task_11 — Open questions, risks & verification checklist

> Status: `tracking` · Depends on: all · Blocks: ship

Living list of unknowns to close before/during build, the real risks, and the end-to-end
verification gate.

## Open questions (must answer before relying on the design)

| # | Question | Resolved by | Why it matters |
|---|----------|-------------|----------------|
| Q1 | Exact Hermes **chat route + streaming schema** (`:8642` SSE vs `:9119`+WS) | task_04 (read fathah), task_02 | The whole UI⇄backend transport |
| Q2 | Does Hermes run **fully local, no Nous account**, with models from our proxy? | task_02, task_05 | Or users are forced onto Nous Portal |
| Q3 | Does ndcode still have upstream opencode **headless `run`/`serve`**? | task_03 (read ndcode source) | If not, Hermes can't drive it autonomously |
| Q4 | **Non-interactive** hub auth for ndcode (key env vs browser `/login`)? | task_03, task_05 | Headless ndcode can't do SSO |
| Q5 | Exact Hermes **config keys** for a custom OpenAI-compatible provider | task_05, task_02 | Points the core at our proxy |
| Q6 | Is the `:8642`/chat path a **stable** Hermes server mode or internals-coupled? | task_04 | Fragility vs maintainability (#1925/#2491) |
| Q7 | Does Hermes support **per-turn abort** over the local API? | task_08 | "Stop" button |
| Q8 | macOS **notarytool profile name** | user input | Notarization is otherwise blocked |
| Q9 | Windows **Authenticode (EV) cert** acquisition | task_09 | SmartScreen-clean installer |
| Q10 | Can `ndcode` be `bun build --compile`'d to a single native binary? | task_03 | Clean bundling |

## Risks

- **Hermes chat API not yet stable** (decoupling tracked upstream #1925/#2491). Wrappers reach
  into internals → our transport may break on Hermes updates. *Mitigation:* pin a Hermes
  version in the provisioning manifest (task_06); isolate the transport behind one adapter.
- **Python provisioning fragility** on user machines. *Mitigation:* uv + standalone CPython
  (no system Python dependency); offline pre-seed option; clear first-run error UX.
- **Two heavy runtimes** (Python for Hermes, Bun/native for ndcode). *Mitigation:* ndcode as a
  single compiled binary; Python only via uv.
- **Bundle size / signing surface** (multiple nested binaries to sign+notarize). *Mitigation:*
  reuse NeuralDeskApp's proven signing; keep the offline Python blob optional.
- **Nous account creep** — risk that some Hermes features assume Nous Portal. *Mitigation:*
  verify account-less local run (Q2) early; gate features that require it.
- **Trademark** — must ship under Neural Deep only (task_10).

## End-to-end verification gate (before calling it done)

- [ ] Clean macOS machine: install signed/notarized app → first run provisions full Hermes via
      uv → headless API healthy → chat streams → Hermes delegates a coding task → ndcode runs
      against the NeuralDeep hub → result streams back. `spctl` clean, opens offline.
- [ ] Clean Windows machine: same flow with signed installer, no SmartScreen block.
- [ ] One hub base URL+key drives **both** Hermes and ndcode (Q2/Q5 verified at runtime).
- [ ] Memory/skills persist across restarts; ndcode delegation visible as tool steps.
- [ ] `THIRD_PARTY_LICENSES.md` present; branding is Neural Deep only.

## Immediate next actions (highest leverage first)

1. **Read fathah/hermes-desktop source** → resolve Q1, Q2, Q5, Q6 (task_04). *This unblocks
   the transport and provisioning.*
2. **Read ndcode source** → resolve Q3, Q4, Q10 (task_03). *This unblocks the coding worker.*
3. Stand up a local headless Hermes pointed at the NeuralDeep hub (task_02 + task_05) as a
   manual spike — prove the backend works before any UI.
4. Then scaffold the Tauri wrapper from NeuralDeskApp (task_07) and wire transport (task_08).
