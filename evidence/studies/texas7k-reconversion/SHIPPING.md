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
