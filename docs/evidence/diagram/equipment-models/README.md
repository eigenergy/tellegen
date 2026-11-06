# Equipment models and detailed DC studies

Captures from the working tree committed with this evidence, in installed Chrome
with 1280 × 844 desktop and 390/320 × 844 touch-emulated viewports. No physical
mobile device or LCC model is qualified by these screenshots.

`apps/web/tests/transmission-equipment.spec.ts` imports two real PowerIO IR fixtures:

- `apps/web/tests/fixtures/transmission-equipment.json`: four AC buses, a zero-angle
  PST, capacitor bank, inactive reactor and reduced fixed-transfer HVDC link.
- `crates/tellegen/tests/data/transmission-mtdc.pio.json`: four AC buses and a meshed
  three-terminal VSC DC network, with explicit ground, terminal mappings, DC
  controls, converter losses and fixed reactive targets.

The six browser tests exercise source settings, mobile scroll containment, DC
solving, qualified AC power flow, AC generation dispatch and canvas PNG export.
The ten existing transmission browser tests also pass. Desktop and touch captures
are examples; device ratings and automatic controls are not inferred from them.

| Capture | Purpose |
| --- | --- |
| `equipment-desktop.png` | Input powers and formulation treatment |
| `equipment-phone.png` | Touch-accessible source inventory |
| `equipment-320.png` | Narrow-screen panel bounds |
| `equipment-export.png` | Main-canvas equipment glyphs in PNG output |
| `mtdc-desktop.png` | Detailed source DC graph |
| `mtdc-phone.png` | Separate converter ports and AC-bus mapping |
| `mtdc-320.png` | DC graph on a narrow phone |

Checks: Rust workspace tests; WASM adapter with `conic,synth`; feature-isolated
`mc-pf`; targeted DC and equipment tests; Clippy with warnings denied for engine,
server and WASM; 60 engine tests; 395 Svelte tests with one existing skip; generated
types; package/web Svelte checks; formatting and diff whitespace checks.

The local optional AC OPF WASM asset is absent. Browser tests used a temporary
Vite configuration excluding its packaging plugin. The ordinary `conic,synth`
WASM was rebuilt and bound using the installed matching wasm-bindgen 0.2.129.
Full `just ci`, joint AC/DC OPF, automatic tap/shunt controls, converter AC-voltage
control and LCC commutation are not qualified by this run. The temporary Vite
configuration and the implementation plan are not committed.
