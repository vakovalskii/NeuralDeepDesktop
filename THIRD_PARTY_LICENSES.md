# Third-Party Licenses

Neural Deep Desktop is a thin wrapper that orchestrates several open-source components.
The entire chain is permissive (MIT / Apache-2.0 / PSF) → **sellable under the Neural Deep
brand**. We ship our own marks only; we do **not** market as "Hermes" or imply Nous
endorsement. Acceptable credit line: *"Powered by Hermes Agent and opencode (MIT)."*

| Component | Role | License | Condition |
|-----------|------|---------|-----------|
| [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) | core harness | MIT © 2025 Nous Research | keep MIT notice + copyright |
| [vakovalskii/NeuralDeepCode](https://github.com/vakovalskii/NeuralDeepCode) (ndcode) | coding worker | MIT (fork of sst/opencode) | keep upstream opencode MIT notice |
| [sst/opencode](https://github.com/sst/opencode) | upstream of ndcode | MIT | notice |
| [fathah/hermes-desktop](https://github.com/fathah/hermes-desktop) | backend patterns reference | MIT | notice if code copied |
| [astral-sh/uv](https://github.com/astral-sh/uv) | Python provisioning | Apache-2.0 | notice |
| python-build-standalone | standalone CPython | PSF / permissive | notice |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | desktop shell | MIT / Apache-2.0 | notice |
| [oven-sh/bun](https://github.com/oven-sh/bun) | ndcode runtime | MIT | notice |
| LiteLLM (optional) | alt local proxy | MIT | notice (only if bundled) |
| React, Vite | wrapper frontend | MIT | notice |

## Notes / gotchas

1. **Trademark ≠ code.** MIT licenses the *code*, not names/logos ("Hermes", "Nous
   Research", "opencode"). Ship under **Neural Deep**.
2. **Models ≠ harness.** These licenses cover harness *code*. Model weights have their own
   licenses — out of scope (models are served remotely via the NeuralDeep hub, not bundled).
3. **Official Hermes Desktop binary is NOT ours to rebrand** (closed prebuilt, Nous-Portal
   credit-gated). We build our own from the MIT sources.
4. **Avoid Open WebUI** for any reuse — recent versions carry a branding-retention clause
   (not pure MIT). Our UI is built in-house (`app/`), so this does not apply.
5. **fathah is community-maintained, not affiliated with Nous** — fine to lift under MIT;
   attribute if code is copied.

## Full MIT notice (applies to the MIT components above)

```
MIT License

Copyright (c) 2025 Nous Research (hermes-agent)
Copyright (c) the sst/opencode authors (opencode, and ndcode as a fork)
Copyright (c) the respective authors (fathah/hermes-desktop, Tauri, Bun, React, Vite, LiteLLM)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

uv is Apache-2.0; python-build-standalone is PSF/permissive — include their full notices in
the bundle's NOTICE file when shipping.

## Verdict

**We may sell Neural Deep Desktop under our own brand**, provided the notices above ship in
the bundle and branding uses only Neural Deep marks. The NeuralDeep hub is ours; reselling
access via the app is permitted (record this in the hub ToS).
