# Hosted pilot browser acceptance

The full six-feeder pilot passed the production browser path on local macOS
ARM64 with Chromium 153.0.8010.12. `browser-validation.json` records the run.
The browser test is opt-in: `apps/web/tests/texas7k-distribution.spec.ts`.

The test verifies API metadata, geographic placement, UI and WebMCP solves,
complete result hash equality against the paired WASM reference, export/import,
a fresh solve with preserved options, result equality after reopening, and a
1% active-power edit at `p1ulv10000`. The smaller hosted-case test covers lazy
loading, catalogue uniqueness, removal/restoration and the same option round trip.

| Measurement | Observed |
| --- | ---: |
| Served PowerIO JSON | 33,952,173 bytes |
| Negotiated gzip body | 1,225,446 bytes |
| Page load through selected case readiness | 5.34 s |
| UI solve through visible terminal results | 1.78 s |
| Load edit through completed UI update | 0.82 s |
| Combined Chromium RSS after loading | 1.46 GB |
| Combined Chromium RSS after solving | 1.65 GB |
| Largest sampled RSS, after study reopen/rerun | 4.80 GB |
| Longest observed main-thread task over the full workflow | 2.19 s |

These are a single local, warm-server run over loopback, not a browser support
matrix or speed guarantee. RSS sums all Chromium processes at checkpoints;
shared pages may be counted multiple times and unobserved peaks may be higher.
The full round trip retains the hosted case and a separate reopened editable
case, so its footprint exceeds a single solve. The first case request was
cancelled when selection was repeated during startup; its zero byte count is
not the served payload size.

The full pilot is functional for desktop evaluation, but its memory and long
main-thread tasks remain material limits. Low-memory/mobile devices are not
qualified. Before deploying broadly, consider a separately validated single
feeder or reduce retained study memory. Do not infer browser memory requirements
from the approximately 1 MB compressed BMOPF source.

Study exports are compact JSON: this pilot's snapshot is approximately 131 MB,
versus approximately 159 MB when indented. Compact output fits the engine's
128 MiB replay limit. Export is disabled while solving. This does not guarantee
that arbitrary larger or edited studies will fit; the engine limit is unchanged.

All changes remain local until reviewed. Production deployment additionally
requires staging the bundle and manifest on the host; image deployment alone
will not install this data. The Texas7k link is case-level; a specific
transmission bus mapping and coupled T&D calculation are outside this change.
