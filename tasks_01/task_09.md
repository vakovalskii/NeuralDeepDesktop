# task_09 — Packaging, signing & notarization (Mac + Windows)

> Status: `build` · Depends on: task_06, task_07 · Blocks: ship

## Goal

Produce signed, installable artifacts on macOS and Windows that bundle the thin shell + the
native bits (ndcode, uv) and provision the harness on first run.

## What's in the bundle

- Tauri shell (`.app` / `.exe`).
- `externalBin`: `ndcode-<triple>` (Bun-compiled native), `uv-<triple>`.
- (Optional offline mode) pre-seeded standalone CPython + Hermes wheels.
- **Not** bundled: the model proxy (external NeuralDeep hub).

## macOS (we already have this working in NeuralDeskApp — reuse)

- **Developer ID Application:** `Valeriy Kovalsky (A933C2TJXU)` — already in keychain.
- Sign with **hardened runtime** (`--options runtime`) + timestamp; entitlements include
  `allow-jit`, `allow-unsigned-executable-memory`, `disable-library-validation`, and any
  device entitlements actually needed (drop unused ones).
- **Every nested binary must be signed**: the main app, the sidecar/host, `ndcode`, `uv`, and
  (if present) the bundled python. Tauri signs `externalBin` automatically when
  `bundle.macOS.signingIdentity` is set — verify each with
  `codesign -dvvv … | grep -E 'Authority|flags'` (expect `flags=…(runtime)`).
- **Notarize** the `.app` and `.dmg`:
  - `ditto -c -k --keepParent App.app App.zip`
  - `xcrun notarytool submit App.zip --keychain-profile "<PROFILE>" --wait` → `Accepted`
  - `xcrun stapler staple App.app`
  - DMG must be **UDIF** — if built via `makehybrid`, convert: `hdiutil convert hyb.dmg
    -format UDZO -o App.dmg`; sign; submit; staple.
  - Verify: `spctl -a -vvv -t exec App.app` / `-t install App.dmg` → "Notarized Developer ID".
- **BLOCKER (parked):** need the **notarytool keychain-profile name**
  (`xcrun notarytool store-credentials …`). Provide it to finish notarization.
- First-run provisioning runs **user-writable** app-data (not inside the signed `.app`), so it
  doesn't break the signature/Gatekeeper.

## Windows

- Tauri MSI/NSIS installer. Code-sign with an Authenticode cert (EV recommended to avoid
  SmartScreen warnings). Sign `ndcode.exe`, `uv.exe`, the app exe.
- `tauri.conf.json` `bundle.windows` (digest sha256, timestamp URL) — already templated in
  NeuralDeskApp.
- Decide cert acquisition (EV cert / signing service) — procurement task.

## Tasks

- [ ] **T9.1** Reuse NeuralDeskApp's macOS signing config + `make bundle` flow; add `ndcode`
      and `uv` as `externalBin`; confirm all nested binaries are signed (hardened runtime).
- [ ] **T9.2** Obtain/confirm the **notarytool profile name**; run notarize+staple for `.app`
      and `.dmg`; verify with `spctl`.
- [ ] **T9.3** Windows: configure MSI/NSIS bundle; acquire Authenticode (EV) cert; sign all
      binaries; produce a signed installer.
- [ ] **T9.4** Validate first-run provisioning works on a clean, signed install on both OSes
      (Gatekeeper-clean on mac; SmartScreen-clean on Windows).
- [ ] **T9.5** CI: per-platform build matrix (mac arm64/x64, win x64) producing signed
      artifacts; pin tool versions (task_06 manifest).

## Acceptance criteria

- [ ] macOS: notarized `.app` + `.dmg`, `spctl` → "Notarized Developer ID", opens offline.
- [ ] Windows: signed installer, no SmartScreen block (with EV).
- [ ] Clean machine → install → first run provisions full harness → chat + ndcode work.

## Primary sources

- NeuralDeskApp macOS notarization runbook (Developer ID, hardened runtime, notarytool,
  UDIF DMG conversion) — already proven for `Neural Deep Proxy.app`.
- Tauri v2 bundling/signing docs (macOS + Windows).
