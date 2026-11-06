# Equipment models and regulated AC/DC studies

Captures from the working tree committed with this evidence, in installed Chrome
at 1280 × 844 and 390/320 × 844 with touch emulation on the narrow viewports.
These are browser checks, not physical-device qualification.

`apps/web/tests/transmission-equipment.spec.ts` imports three PowerIO IR fixtures:

- `apps/web/tests/fixtures/transmission-equipment.json`: four AC buses, a zero-angle
  PST, capacitor bank, inactive reactor and reduced fixed-transfer HVDC link.
- `crates/tellegen/tests/data/transmission-mtdc.pio.json`: four AC buses and a meshed
  three-terminal VSC DC network with explicit ground, terminal mappings, controls,
  converter losses and fixed reactive targets.
- `apps/web/tests/fixtures/transmission-acdc-controls.json`: the detailed fixture
  with declared converter P/DC-voltage/Q bounds and converter B regulating AC bus 2
  to 1.02 pu. The example limits are test inputs, not inferred equipment ratings.

The nine equipment browser tests exercise source inspection, automatic regulators,
converter voltage control, DC solve, fixed-transfer AC dispatch and actual joint
AC/DC optimization in the optional WASI worker. They check mobile containment,
1.02 pu at the regulated bus, declared/unspecified bound counts and PNG export.
The ten existing transmission diagram browser tests also pass.

| Capture family | Purpose |
| --- | --- |
| `equipment-{desktop,phone,320}.png` | Source inventory and fixed-transfer interpretation |
| `equipment-export.png` | Main-canvas equipment glyphs in PNG output |
| `mtdc-{desktop,phone,320}.png` | Detailed DC graph and separate AC terminal mappings |
| `regulators-{desktop,phone,320}.png` | Regulated shunt and unreachable PST target on a radial fixture |
| `converter-voltage-{desktop,phone,320}.png` | VSC Q adjustment and the reached 1.02 pu target |
| `joint-acdc-{desktop,phone,320}.png` | Joint OPF objective and AC voltage results |

The main diagram's ordinary calculation rejects unreduced detailed converters.
Its notice points to Equipment for AC/DC studies. Inspector calculations use the
original import and report separately from the main diagram; no main-diagram
result is manufactured by the inspector. Positive converter Q is absorption.

Native checks include tap/shunt limits, discrete and continuous controls, native
versus normalized phase control, Q-limit saturation, capability-curve interpolation
without extrapolation, LCC power factor at both ends and current-direction limits.
Joint tests compare the generation objective with fixed-transfer AC OPF, verify
power/loss balance, exercise VSC voltage control and LCC operation, and check the
analytic Jacobian and Hessian against finite differences. LCC qualification is
native; these browser screenshots use VSCs.

Validation for this update: Rust workspace tests; 436 nonlinear-feature library
tests passed (two existing ignores); isolated `mc-pf`; 60 WASM adapter tests and
its integration test; optional WASI ABI test; Clippy with warnings denied; 64 engine
and 395 Svelte tests (one existing skip); package/web type checks; production web
build; formatting; PowerIO pin and optional-backend license-boundary guards.
Full `just ci` was not run. One capture attempt overlapped a production rebuild and
failed during case import; the complete nine-test capture run passed after builds
finished. The final captures come from that stable run.

Both the ordinary WASM and the optional POUNCE WASI asset were built locally.
The ordinary Vite configuration served the separate optional asset; no packaging
plugin was disabled. Reproduce with `npm run wasm:acopf`, the ordinary engine/WASM
build, and `PUBLIC_TELLEGEN_ACOPF_WASM_URL=/experimental-acopf/tellegen_acopf_wasi.wasm`.
Run the equipment and transmission diagram Playwright specs against that server.

Scope: LCCs use explicit fixed power factor; commutation dynamics and firing-angle
control are unsupported. Joint OPF is local, uses fixed taps/shunt sections and
hard converter voltage schedules, and requires explicit converter operating bounds.
Missing line ratings remain unchecked. Neither the implementation plan nor local
Playwright configurations are committed.
