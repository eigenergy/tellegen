# Texas7k distribution preparation

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
