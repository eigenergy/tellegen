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

### Browser engines and narrow viewport qualification

The final serial matrix uses the same stable bundle, server and production
frontend. All three projects pass hosted loading, UI/WebMCP solves, export,
reopen, rerun with identical options/results, and a completed load edit. The
edit assertion now waits for an advanced accepted-result revision, a converged
result and `solving=false`; it cannot finish before the debounced edit starts.
All final screenshots were inspected.

| Browser / viewport | Load | UI solve | Completed edit | Sampled RSS sum |
| --- | ---: | ---: | ---: | ---: |
| Chromium 156, 1280 × 800 | 8.559 s | 4.102 s | 0.738 s | 3.21 GB |
| Playwright WebKit 27.2, 1280 × 800 | 1.357 s | 1.299 s | 0.565 s | Unavailable |
| Chromium Pixel 7 emulation, 412 × 839 | 6.729 s | 2.448 s | 0.374 s | 3.63 GB |

These are single local measurements with a warm server, not controlled
cross-engine benchmarks. The memory figure is the sum of process RSS at
checkpoints and can count shared pages repeatedly. WebKit exposes neither the
Chromium process-memory protocol nor long-task observations here; those
metrics are recorded as `null`, not zero. Reports are
`browser-validation-chromium-desktop-2026-11-07.json`,
`browser-validation-webkit-desktop-2026-11-07.json`, and
`browser-validation-mobile-emulation-2026-11-07.json`.

The first WebKit attempt over plain loopback HTTP stopped before hydration:
the unchanged production CSP upgraded asset requests to HTTPS. The final
matrix therefore uses a loopback TLS proxy to the actual native server and
accepts its local self-signed certificate only in the test browser. No
production policy or solver behavior was changed for qualification.

Run this optional matrix against an existing staged HTTPS server:

```sh
TELLEGEN_TEXAS7K=1 TELLEGEN_TEXAS7K_URL=https://your-staged-server \
  npm --workspace tellegen-frontend exec -- playwright test \
  --config playwright.texas7k.config.ts
```

For a local self-signed TLS endpoint only, also set
`TELLEGEN_TEST_SELF_SIGNED_TLS=1`. The configuration runs one browser at a time
and needs installed Chromium and WebKit engines. Firefox was not installed
and was not tested. The Pixel 7 project verifies narrow viewport and touch
emulation on the desktop host; **physical low-memory mobile devices remain
unqualified**. This evidence supports the tested desktop browser engines and
the emulated layout, not an unrestricted mobile support claim.

### Published immutable bundle

The [v0.6.2 dataset archive](https://github.com/eigenergy/tellegen/releases/download/v0.6.2/texas7k-p1uhs0_1247-a594e609e0a0cc31.tar.gz)
contains the immutable bundle and its distribution-case manifest. An independent
download verified all 17 archive files, all 15 bundle checksums, the license and
attribution files, and equality between the compressed and uncompressed module.
The archive is 4,879,300 bytes with SHA-256
`99f98f402511452dfc46684f791852482668dece4b3acf4aa16cb46cef26e8fd`.
The bundle checksum manifest has SHA-256
`a594e609e0a0cc3108b1cf58b649052d6be70e805804ff70a2b75158d2f5d8c4`.
`release-archive-2026-11-07.json` records the public download and verification.
The upstream CC-BY-3.0-US license and attribution remain in the bundle.

### Production acceptance and compiler alignment

The immutable bundle is now active at `https://tellegen.dev`, with the previous
manifest state retained for rollback. Activation held the deployment lock,
preserved every existing catalogue entry and verified that the running image
did not change. All five cases are healthy and none is unavailable. The
distribution manifest has SHA-256
`385ba178594cf1b774ea4338f97b9df89fced0444dddc48a0c335da68f1b5552`.

After deployment of the shared frontend at `b85c50f3`, the public Chromium
acceptance passed transmission association, hosted load, UI and WebMCP solves,
export, saved reopen, an exact-result rerun with identical settings, and a
completed converged load edit. Normal HTTPS certificate validation remained
enabled. `production-browser-validation-2026-11-07.json` records the exact
tested image, source revision, deployment workflow, health and measurements.
The source comes from the workflow's tested-source fields; a workflow-run
metadata revision alone is not deployment provenance.

This single public run loaded in 9.503 seconds, solved through the UI in
2.178 seconds and completed an edit in 0.695 seconds. The largest sampled
browser process RSS sum was 4.04 GB; this is not a continuous peak or a
physical mobile qualification, and shared pages can be counted repeatedly.
The screenshot was inspected.

The original full-result hash did not match the first production binary.
`production-artifact-comparison-2026-11-07.json` retains the original reference
and records the isolation: each exact binary reproduces its own full result
on both the immutable hosted input and the exported retained input, through
both the direct and browser paths. All topology, identifiers, options,
qualification fields and 23 iterations are unchanged. The maximum complex
voltage difference is about 10 nanovolts; microampere current differences
concentrate in cancellation around existing one-microohm switch/fuse
primitives. Both results satisfy the unchanged convergence criteria.

WASM producer metadata identifies the build difference: the floating Docker
image used Rust 1.99.1 while local and CI builds pin Rust 1.96.0. Both Docker
Rust stages now select `rust:1.96.0-slim-trixie`, aligning the compiler while
retaining the runtime's Debian release. The browser test first hashes the
actual downloaded WASM and then requires its independently qualified exact
result hash. Unknown artifacts fail; neither the old reference nor a general
numerical tolerance is replaced. The pinned-compiler image still requires its
own post-merge production check.

The published `@tellegen/engine@0.8.0` archive was then downloaded independently,
checked against its registry integrity, and executed using its own glue and
WASM on the unchanged hosted input and options. Its Rust 1.96.0 artifact
produces the original full-result hash exactly (23 iterations, 55,972
terminals). `registry-artifact-validation-2026-11-07.json` records the package
archive digest, source/workflow provenance fields, WASM producer metadata and
result. That actual artifact was added to the exact-result qualification map;
a different final Docker artifact still requires independent qualification.

The final Docker-built production core was independently qualified after the
pinned-compiler deployment. It has a distinct fingerprint from the registry
archive but the same Rust 1.96.0 producer version and exactly the original
complete scientific result on the unchanged input/options. The actual public
WASM, host checksum, source revision and image are recorded in
`production-compiler-validation-2026-11-07.json`; its exact artifact/result
pair is now included in the strict browser acceptance map.
