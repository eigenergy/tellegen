# Generated-grid hand-off to the viewer (#261)

Where the time goes between "the generated grid is ready on the synthetic
worker" and "the viewer shows its first DC OPF result", for a 3,000-bus
grid, and what changed.

## How to run

```sh
npm ci
npm run wasm && npm run wasm:acopf && npm run build:web   # the Playwright config serves the built app too
validation/evidence/synth/handoff/run.sh            # the hand-off as it is now
validation/evidence/synth/handoff/run.sh serial     # the hand-off as it was before #261
TELLEGEN_SYNTH_BENCH_GPU=1 validation/evidence/synth/handoff/run.sh   # hardware GL
```

`run.sh` runs the opt-in benchmarks of
`apps/web/tests/synth-handoff-bench.spec.ts` in Playwright's Chromium. They
print one table per test: the first sample (cold) and the median, minimum
and maximum of the rest. `TELLEGEN_SYNTH_BENCH_RUNS` sets the number of
samples after the cold one (default 5), `TELLEGEN_SYNTH_BENCH_BUSES` the bus
count (default 3000), and `TELLEGEN_SYNTH_BENCH_CHINA=1` generates MapToGrid's
request for China at its default density in the viewer test.

Each engine worker answer carries when the worker took the request, how long
the engine ran and when the answer was posted
(`received_at`, `worker_ms`, `posted_at`), on the clock the page reads. The
benchmark records every worker request from the page and joins the two, so a
round trip splits into the copy to the worker, the engine's time, and the
copy back.

Two tests:

- **engine**: the public engine API on a bare page, from the grid being ready
  to the first DC OPF result parsed on the page.
- **viewer**: tellegen's shell with its map, opened the way MapToGrid opens a
  grid (`Controller.openBalancedModule` with the payload and
  `formulation: 'dcopf'`), from the grid being ready to the first frame that
  shows the solved DC OPF card. Every sample is a newly loaded page.

`serial` is the hand-off before this change: `session.ingest()`, then the
viewer builds its Study from `payload.module_json`. `ahead` is
`session.ingestWithStudy('dcopf')` with its Study passed to
`openBalancedModule`.

## Results

16 November 2026. Apple M5 Pro (18 cores), 64 GB, macOS 26.6.2, Chromium
158.0.8127.18 (Playwright 1.65.0), Node 26.13.1. The grid is
`packages/engine/tests/fixtures/handoff-request.json` at 3,000 buses: 4,812
branches, 12.54 M characters of module text. Medians of 9 samples after one
cold sample, with the minimum and maximum. Other work was running on the
machine. The full output is in `results-2026-11-16.md`.

### Where the time went (before)

Engine test, headless shell, ms:

| stage | median | min to max |
|---|---:|---:|
| payload built and serialized on the synthetic worker | 79 | 74 to 97 |
| copy of the payload, synthetic worker to page | 3 | 2 to 5 |
| payload parsed on the page | 23 | 20 to 28 |
| copy of the module text, page to study worker | 2 | 2 to 3 |
| Study built on the study worker: module parsed, DC OPF solved | 203 | 194 to 255 |
| DC OPF result fetched and parsed | 7 | 6 to 9 |
| **grid ready to first DC OPF result** | **313** | 304 to 396 |

- The two copies between threads take about 5 ms together. A binary module
  or a transferable buffer in place of JSON text would save at most that.
- The stages ran one after another on three threads: the study worker waited
  for the payload to be built and parsed before it could start.
- The payload held the module text as a JSON string inside JSON: the module
  was serialized, escaped into the payload, and the page parsed all 18.3 M
  characters to get the text back.
- Timed in Node on the same wasm, parsing the module text is about 45 ms of
  the Study build and serializing it again for the Study's saves about 29 ms.
  The rest is the Study's own setup and the DC OPF.

Viewer test: the same stages, plus the page. With hardware GL the page adds
no long task and the hand-off was 355 ms. In Playwright's headless shell,
which draws WebGL in software, the page's main thread was busy for about
1.4 s after the grid opened, and the Study's answer, ready after about
340 ms, waited about 1 s for it. A 100-bus grid waits as long, so this is
the first draw of the grid's map layers in software, not the size of the
grid. It is most of what an application measures in headless Chromium.

### Before and after

ms, median (min to max):

| stage | before (`serial`) | after (`ahead`) |
|---|---:|---:|
| **engine, headless shell** | | |
| module serialized on the synthetic worker | in the payload | 26 (26 to 27) |
| payload built and serialized | 79 (74 to 97) | 31 (30 to 32) |
| copies, synthetic worker to page | 3 (2 to 5) | 2 (2 to 2) and 1 (1 to 1) |
| payload parsed on the page | 23 (20 to 28) | 5 (5 to 5) |
| copy of the module text, page to study worker | 2 (2 to 3) | 2 (1 to 2) |
| Study built: module parsed, DC OPF solved | 203 (194 to 255) | 187 (185 to 190) |
| DC OPF result fetched and parsed | 7 (6 to 9) | 6 (5 to 7) |
| grid ready to payload on the page | 104 (96 to 130) | 65 (64 to 67) |
| **grid ready to first DC OPF result** | **313** (304 to 396) | **223** (220 to 225) |
| **engine, hardware GL** | | |
| grid ready to payload on the page | 94 (91 to 95) | 66 (63 to 68) |
| **grid ready to first DC OPF result** | **289** (282 to 293) | **221** (218 to 227) |
| **viewer, hardware GL** | | |
| grid brought to the page | 110 (108 to 114) | 80 (78 to 80) |
| grid ready to Study built | 334 (331 to 345) | 246 (243 to 248) |
| page main thread in long tasks | 0 | 0 |
| **grid ready to solve card painted** | **355** (353 to 364) | **266** (261 to 269) |
| **viewer, headless shell (software GL)** | | |
| grid brought to the page | 112 (108 to 118) | 81 (79 to 83) |
| grid ready to Study built | 337 (332 to 361) | 245 (244 to 252) |
| Study answer waits for the page | 967 (911 to 1588) | 992 (979 to 1394) |
| page main thread in long tasks | 1364 (1297 to 2006) | 1322 (1303 to 2250) |
| **grid ready to solve card painted** | **1519** (1446 to 2167) | **1439** (1423 to 1901) |

In the `ahead` column the Study builds while the payload is prepared, so its
rows overlap and do not add up to the total.

The engine hand-off is 29% shorter in the headless shell and 24% shorter
with hardware GL, and the viewer with hardware GL 25% shorter. In the
headless shell the viewer's median is 5% lower and the two ranges overlap:
there the page's first software draw decides when the result appears, and
this change does not touch it.

## What changed

- `BrowserSynthSession.ingestWithStudy(formulation)` asks the synthetic
  worker for the module text, gives it to the study worker at once, and only
  then asks for the rest of the payload. The Study builds while the payload
  is prepared and parsed.
- The rest of the payload comes without the module text
  (`SynthSession.ingest_view` in the wasm), so the module is serialized once
  and the page no longer parses its text.
- `Controller.openBalancedModule` takes that Study through a `study` option
  and the case's first solve uses it instead of building another.

`ingest()`, `createStudy()` and `openBalancedModule` without `study` behave
as before.

## Not done

- A Study built on the synthetic worker (`intoStudy` on a shared host) would
  skip the 45 ms module parse, but the synthetic worker is terminated when a
  generation is cancelled, which would take the open grid's Study with it.
  Building it ahead on the study worker gets a similar total without that.
- A binary module between workers: the copies are about 5 ms.
- The Study still serializes the module once more for its saves (about
  29 ms) and solves a DC OPF the generator has already solved.
- The viewer fetches the base solution twice (about 4 ms each with hardware
  GL).
- The first software draw of the map layers in headless Chromium.
