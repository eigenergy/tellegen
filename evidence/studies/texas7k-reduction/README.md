# Texas7k distribution JSON reduction prototype

This experiment reduces the storage representation of the complete
`p1uhs0_1247` six-feeder pilot. It does not extract a feeder, eliminate buses,
simplify electrical equipment, round numbers, or change the operating point.
The original BMOPFDraftData files remain unchanged.

## Results

Sizes are decimal MB; gzip is level 9 with a reproducible zero timestamp.

| Representation | JSON bytes | Gzip bytes |
| --- | ---: | ---: |
| Original formatted BMOPF | 30,032,660 | 1,557,897 |
| Original, whitespace removed only | 17,386,523 | 1,479,269 |
| Reduced prototype, including compact provenance | 12,100,718 | 979,383 |

The prototype is **59.7% smaller uncompressed and 37.1% smaller compressed**
than the original. Against a minified original, structural reduction saves
another 30.4% uncompressed. These sizes exclude the optional 389,171-byte
provenance sidecar, copied license/notice, and solver options. The full removal
sequence and file hashes are in `size-report.json`.

The generated files are under the ignored operator-data directory
`data/prototypes/texas7k-reduced/`:

- `p1uhs0_1247.reduced.bmopf.json`, plus its `.gz` copy: the reduced case.
- `p1uhs0_1247.reduced.pio.json`: a portable module emitted by the browser WASM
  ingestion path, suitable for the new hosted distribution catalogue. It is
  33,952,173 bytes, compared with 46,965,169 bytes for the module generated from
  the original BMOPF. BMOPF and PowerIO IR are different representations; the
  12.10 MB figure is the BMOPF file, not the hosted IR payload.
- `reduction-provenance.json`: original metadata, line-code aliases, removed
  unused-code names, and all 2,483 derived-location associations.
- `License.md`, `SOURCE_NOTICE.md`, `tellegen_options.json`: copied from the
  pilot without modification.

The original prototype files remain unregistered. The reproducible staging
script in `scripts/datasets/` validates the pinned source and registers a separate
operator data bundle. No production deployment is performed.

## What can be removed or shared

1. Whitespace accounts for 12.65 MB of the original file. It has a much smaller
   effect after transport compression.
2. `extras.geojson` repeats all 22,081 bus locations already present in
   `bus[id].longitude` and `latitude`. It has no line routes. Removing that
   collection saves 3.43 MB of compact JSON. The script first checks every
   coordinate, rejects non-Point features, and retains the derived-location
   associations separately.
3. Of 2,440 line codes, 30 are unused. The 2,410 referenced codes contain only
   **24 distinct complete records**. Referencing one existing representative
   of each exact record saves a further 1.44 MB after unused codes are removed.
   Equality includes impedance and shunt matrices and ratings. All physical
   line IDs, lengths, terminal mappings, and line-specific ratings remain.
4. All 15,299 loads explicitly state `model: "CONSTANT_POWER"`. This field is
   optional in the pinned BMOPF 0.2.0 proposal schema, and both Tellegen's raw
   validation and PowerIO's reader default its absence to constant power.
   Omitting those labels saves 382,475 bytes. No ZIP/exponential parameters
   are present; the reducer rejects them rather than guessing.

Attribution, schema identity, frequency, terminal conventions, conversion notes,
and the nominal-snapshot definition remain. Their combined size is small.

## Fields deliberately retained

- Transformer `s_rating` and nominal voltages are used by PowerIO to construct
  winding impedance bases. They are not disposable rating annotations.
- Winding resistances/reactances, tap ratios, and no-load shunts determine the
  power flow and losses, including the split-phase leakage-star equivalents.
- `v_nom`, load configuration, terminal maps, grounding, and terminal conventions
  are needed for interpreting voltages and connections. They are retained even
  where a particular constant-power solve may not use every value directly.
- Line `i_max` and transformer ratings support loading interpretation. Removing
  them merely because they do not constrain a PF solve would make a worse demo.
- Matrix precision and equipment identifiers are unchanged. The reducer makes
  no assumptions about omitting individual zero-valued matrix entries.

The audit used PowerIO 0.11.3's local `bmopf/read.rs`, `bmopf/geo.rs`, and
vendored 0.2.0 schema, plus Tellegen's `mc_pf/input.rs`. It does not fetch or
change a schema. Like the original, the reduced BMOPF uses direct bus coordinate
extensions. No claim of strict whole-document JSON Schema conformance is made;
PowerIO/Tellegen ingestion passes with zero diagnostics.

## Validation

`reduce.py` asserts that electrical tables match after restoring the optional
constant-power label, and that every physical line resolves to its original
complete line-code record. Bus coordinates are checked exactly without rounding.

`native-validation.json` records a paired run with the freshly checked
`cargo build --release --locked -p tellegen --example mc_pf --features mc-pf`
binary. Both runs converge in 23 iterations, with one factorization and the
same 40,700-dimensional matrix. **Every result field and the full output bytes
match**, including 55,986 terminal records and 128,444 element-port records.
Maximum terminal complex-voltage difference is zero.

`wasm-validation.json` repeats ingestion and creates a retained `McPfSession`
from each resulting portable module using the installed browser WASM build.
Both cases have zero diagnostics, support AC PF, and place all 22,081 buses in
geographic space. **The entire network graph and every PF result field match.**
Original and reduced outputs are compared within each backend; native and WASM
rounding need not match each other.

There is one intentional non-electrical metadata difference: parsing only
direct longitude/latitude preserves global Source provenance but does not add
the redundant GeoJSON reader's per-point `kind: "source"` attribute. The display
payload is otherwise exactly equal, including geographic space, coordinates,
names, and IDs. The comparison permits only that precise difference; it does
not discard arbitrary metadata from its comparison.

Validation uses the pilot's supplied options (`voltage_envelope: false`,
`tolerance: 1e-7`, `absolute_kcl_tolerance: 1e-5`, `max_iterations: 500`). These
options are a companion file, not embedded in the BMOPF or portable module;
hosting the module alone does not configure those solver defaults.

Recorded timings and peak memory are single-process observations, not a browser
performance benchmark. The reduced WASM validation process still peaked around
970 MB: smaller JSON does not remove the solver's full-network memory needs.
This experiment did not re-run the independent OpenDSS reference; it proves
equivalence to the original pilot through Tellegen's native and WASM paths.

## Reproduce

From the Tellegen repository root, supply the local pilot path:

```sh
pilot=/path/to/BMOPFDraftData/output/Texas7kPilot
prototype=data/prototypes/texas7k-reduced
evidence=evidence/studies/texas7k-reduction

python3 "$evidence/reduce.py" "$pilot/p1uhs0_1247.bmopf.json" "$prototype"
cargo build --release --locked -p tellegen --example mc_pf --features mc-pf
python3 "$evidence/validate-native.py" target/release/examples/mc_pf \
  "$pilot/p1uhs0_1247.bmopf.json" "$prototype/p1uhs0_1247.reduced.bmopf.json" \
  "$prototype/tellegen_options.json" "$evidence/native-validation.json"
node "$evidence/validate-wasm.mjs" packages/engine/dist/wasm-pkg \
  "$pilot/p1uhs0_1247.bmopf.json" "$prototype/p1uhs0_1247.reduced.bmopf.json" \
  "$prototype/tellegen_options.json" "$evidence"
cp "$prototype/size-report.json" "$evidence/size-report.json"
```

The WASM check requires an already built engine package. It writes the portable
prototype module as well as the compact reports. The native check deletes its
large temporary result dumps after comparing them. Each report identifies the
actual binary or WASM artifact by SHA-256.
