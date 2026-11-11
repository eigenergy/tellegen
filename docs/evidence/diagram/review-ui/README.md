# Transmission UI review fixes

Historical UI evidence for PR #300. Screenshots are preserved in [the original capture commit](https://github.com/frederikgeth/tellegen/tree/ddf2b17abad6e3a783dd56dcb8a74344090b0a19/docs/evidence/diagram/review-ui); filenames below refer to that archive. These screenshots
supersede earlier PR screenshots for control layout and explanatory text. The older CATS and
Texas7k screenshots remain evidence for their earlier full-data qualification, not this UI.

The deterministic cases are defined in `apps/web/tests/transmission-diagram.spec.ts`:

- `phone-operating.png`: two-bus DC OPF at 100% utilization, with zero overloads.
- `phone-economics.png`: four-bus DC OPF; all views are reachable through the labelled selector,
  and the collapsed panel hides price-scale settings.
- `phone-explanations.png`: the same case, with the economics glossary expanded and scrolled into
  view. This panel scrolls within its height limit so the network remains accessible.
- `desktop-operating.png`: four-bus DC OPF, with the display-only warning set to 10% by the test.
- `desktop-ac.png`: the same four-bus case in AC power flow, with voltage and loading checks.
- `unknown-thermal.png`: a mocked legacy backend result with no branch constraint metadata;
  price differences do not produce a claim about which thermal limits bind.

The browser suite also checks missing flows and a retained result from a different formulation,
keyboard radio navigation, 44-pixel phone controls at 390 and 320 pixels wide, geographic and
schematic layouts, equipment selection, warning preferences and PNG exports.

Validation: 10 browser tests passed; the Svelte package suite passed 389 tests with one existing
skip; `npm run check:web` and `npm --workspace @tellegen/svelte run check` passed with no Svelte
diagnostics. The web check includes eight study-goal tests. Changed TypeScript and Svelte files
passed Prettier; `git diff --check` was clean.

Normal reproduction after building the app and its assets:

```sh
npm run build:web
npm --workspace tellegen-frontend run test:browser -- tests/transmission-diagram.spec.ts
```

This capture used installed Chrome and a temporary Vite configuration because this checkout
lacks `target/experimental-acopf/tellegen_acopf_wasi.wasm`. The temporary configuration imported
`apps/web/vite.config.ts` and excluded only the `tellegen-experimental-acopf` asset-packaging
plugin. No replacement solver was installed, and AC OPF was not exercised. The test runner used
that dev server at `http://127.0.0.1:5173`, with `channel: 'chrome'` and a 1280 × 800 desktop viewport.
Phone tests set 390 × 844 with touch emulation. This is not physical-device qualification.

`capture.json` records file hashes and the source test-output paths. Images are unmodified
Playwright screenshots, including case-import notices where present.
