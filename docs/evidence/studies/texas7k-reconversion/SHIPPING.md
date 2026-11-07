# Compact main-island shipping bundle — 30 October 2026

The PowerIO 0.11.7 main-island derivative now has a compact, validated hosting
bundle under `data/shipping/texas7k-main-island/`. Its
`distribution-cases.json` points to the content-addressed bundle
`distribution/texas7k-p1uhs0_1247-6f60e4cd807b09b3/`.

| Representation | Original bytes | Compact bytes | Compact gzip bytes |
| --- | ---: | ---: | ---: |
| BMOPF exchange JSON | 44,911,506 | 13,693,080 | 1,096,525 |
| Hosted PowerIO module | 69,245,395 | 36,692,131 | 1,260,618 |

Sizes are decimal bytes. The hosted asset is
`p1uhs0_1247.reduced.pio.json`; its gzip companion is included. The BMOPF
file is an exchange-format companion and is not the hosted payload.

The reducer removes duplicate GeoJSON bus Points and explicit default
constant-power labels, and shares identical line-code records. PowerIO 0.11.7
gives every effective line code a distinct source label; those labels are
retained in `reduction-provenance.json` so the electrical records can share
storage. There are 1,510 distinct complete electrical records instead of
16,819 separately labeled records. The sidecar is 3,412,461 bytes and is kept
separately from the hosted module. It also retains aliases and the original
derived-location associations. Source labels are restored when verifying
each original complete line-code record.

The bundle retains all 22,076 buses, 16,819 lines, 7,756 transformer units and
15,299 loads of the main island. All electrical values and coordinates are
preserved without rounding. It includes the source/license notices,
island-removal records, pinned input checksums, PF defaults, checksums and
paired browser validation evidence.

## Validation

- Native original/compact runs produce identical complete PF results and
  identical result bytes: 55,972 terminals, 128,426 equipment ports, 23
  iterations, one factorization and a 40,685-dimensional matrix. Maximum
  complex terminal-voltage difference is zero.
- Browser WASM ingestion places every bus geographically, reports zero
  diagnostics and supports PF directly. The graph and all PF result fields
  match exactly. The display differs only in the previously documented
  redundant per-point source tags; coordinates and global provenance match.
- The compact electrical projection passes the pinned BMOPF 0.2.0 proposal
  schema. Every staged file checksum is verified. Three staging failure-path
  tests pass, including corrupted prepared input preserving the old manifest.

`shipping-native-validation.json`, `shipping-wasm-validation.json`,
`shipping-size-report.json` and `shipping-summary.json` retain compact evidence.
Native/WASM artifact hashes identify the actual binaries; the browser artifact
was the existing build at `packages/engine/dist/wasm-pkg` in the primary
checkout and produces PowerIO 0.11.7 modules. This paired WASM check does not
claim a fresh browser build or a full browser UI acceptance run. Native and
WASM numerical rounding may differ across backends; each backend's original
and reduced cases are compared against each other.

## Reproduce

The source-preparation lock is
`scripts/datasets/texas7k-main-island-source.json`. From a checkout containing
the generated main-island files and a built browser engine:

```sh
python3 scripts/datasets/stage-texas7k.py \
  data/prototypes/texas7k-main-island data/shipping/texas7k-main-island \
  --main-island --wasm /path/to/built/wasm-pkg
```

The staging script verifies the prepared source, reduces it with
`--externalize-linecode-sources`, reruns paired browser checks, and registers
the hosted module while preserving other manifest entries. Bundle hashes
include timing reports, so fresh runs can have different directory names;
module bytes are reproducible with the same inputs and WASM artifact.

This evidence covers local operator-data preparation. Production deployment
is a separate operation.

## Refreshed integration — 3 November 2026

The #169 refresh includes #268's retained-input and qualification changes and
main through `70754a7`. The existing checksum-pinned main-island source was
staged again with the matching engine WASM. The engine Rust/WASM source is
unchanged from the tested #268 integration at `680f6a8`.

`shipping-wasm-validation-2026-11-03.json` records the current artifact hashes
and the paired original/compact checks. All PF result fields remain exactly
equal between the two representations, with 55,972 terminals, 128,426 ports,
23 iterations and one factorization. Every input bus coordinate matches;
display differences remain limited to the optional per-point source tags.
The result hash includes the new load-model qualification fields and current
solver defaults, so it is separate from the historical 30 October report.

This is a fresh local staging check. Production data is managed separately.

The production-build Chromium acceptance flow also passes against the freshly
staged native server: hosted loading, UI/WebMCP solves, export, saved reopen,
rerun and a load edit. The result hash matches the paired WASM report exactly.
`browser-validation-2026-11-03.json` records the single local run. Timings are
loopback measurements, not device support guarantees. The memory figure sums
Chromium process RSS at checkpoints and can count shared pages more than once;
it is not a continuously measured peak or a WASM heap measurement.

## Reconstructed and checked — 7 November 2026

The previously local prepared inputs were reconstructed from the public pinned
BMOPFDraftData revision. All 62 source files, the full conversion, and all six
prepared main-island input hashes match their existing locks exactly. The new
`scripts/datasets/prepare-texas7k.py` uses the published PowerIO 0.11.7 Python
binding and the original OpenDSS dependencies, avoiding historical native
builds. It verifies the current browser WASM results before installing the
prepared inputs. The shipping engine remains on PowerIO 0.11.9.

`reconstruction-2026-11-07.json` records the complete source and binary proof.
The locally staged immutable bundle is
`distribution/texas7k-p1uhs0_1247-a594e609e0a0cc31/`. Its hosted module is still
36,692,131 bytes, or 1,260,618 bytes in the staged gzip companion. All 15 bundle
file checksums were verified. `shipping-wasm-validation-2026-11-07.json` confirms
exact graph, coordinate and PF result preservation through reduction, zero
diagnostics, and the same result hash as the prior accepted browser run.

The actual native server served the production frontend and this staged case
for a fresh Chromium acceptance run. Hosted load, UI and WebMCP solves, export,
saved reopen, rerun with unchanged options, and a load edit all passed. The
screenshot was inspected. `browser-validation-2026-11-07.json` records a
7.849-second load, 1.044-second UI solve and 1.057-second edit on loopback.
The largest sampled browser process RSS sum was 3.60 GB; shared pages may be
counted more than once. The longest observed main-thread task was 3.317 seconds.
These are desktop measurements, not a mobile performance qualification.

The exact same unmodified PowerIO 0.11.9 hosted IR was also solved with the
already-built PowerIO 0.12.0-rc.1 candidate at the revision recorded in
`candidate-compatibility-2026-11-07.json`. Every PF result field, the settings
schema, and explicit solver options match the stable build exactly. Both
actual WASM binary hashes are retained. No candidate-generated module was
staged, and this check does not imply that a 0.11 reader accepts new 0.12 IR.

Production promotion is separate: copy the immutable bundle, preserve existing
distribution manifest entries, replace only this case's entry atomically, and
retain the previous manifest for rollback. These records describe local
preparation and acceptance, not a production deployment.
