# Texas7k distribution preparation

## Main energized island (PowerIO 0.11.7)

The current main-island derivative retains all six feeders and 15,299 loads,
with five unsupplied buses and three lines removed. After reproducing it using
`evidence/studies/texas7k-reconversion/README.md`, prepare the shipping bundle:

```sh
python3 scripts/datasets/stage-texas7k.py \
  data/prototypes/texas7k-main-island data/shipping/texas7k-main-island \
  --main-island --wasm /path/to/built/wasm-pkg
```

`texas7k-main-island-source.json` pins the exact prepared input and its license,
modification notice, island-removal records and solver options. This mode reads
prepared files locally and checks every hash before reduction or manifest
changes. It removes redundant geometry, shares identical electrical line-code
records, and preserves their per-line source labels in a provenance sidecar.
No electrical values are rounded. The original full-pilot mode below remains
available for reproducing the historical bundle.

For the general staging wrapper, set
`TELLEGEN_TEXAS7K_MAIN_ISLAND=/path/to/prepared-main-island` when invoking
`scripts/stage-data.sh`. This takes precedence over the historical
`TELLEGEN_BMOPF_REPO` setting.

The destination contains `distribution-cases.json` and a content-addressed
`distribution/texas7k-p1uhs0_1247-*/` bundle. Its
`p1uhs0_1247.reduced.pio.json` is the hosted asset; the corresponding `.gz`
file is also included. The smaller `.reduced.bmopf.json` is an exchange-format
companion, not the hosted module. Checksums, source lock, removal provenance,
license notices, geographic verification and paired browser WASM solve evidence
are bundled together. This command prepares local operator data; deployment is
a separate operation.

## Historical full pilot

Run `python3 scripts/datasets/stage-texas7k.py /path/to/BMOPFDraftData data`
from a checkout with a built browser engine (`npm run wasm && npm run build:engine`).
The source lock pins committed input bytes and companion notices/options.
No download, commit, push, or deployment is performed by this script.

The preparation runs the reduction and paired WASM verification in
`evidence/studies/texas7k-reduction`. Failed validation leaves the existing
manifest unchanged. Other case entries are preserved. Bundles include checksums,
source revision, provenance, licenses and validation reports; their names hash
the whole bundle, including run-specific timing evidence. Network module bytes
are reproducible with the same source lock, reducer and WASM artifact.

Retain the previous manifest and bundles for rollback. One staging process
should own a destination directory at a time. See `docs/src/deployment.md` for
host preparation and the full-pilot browser check.

Test failure handling with `python3 scripts/datasets/test_stage_texas7k.py`.
