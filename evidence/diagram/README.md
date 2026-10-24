# Operating-point overview at scale

How long the multiconductor Studies overview (`McWorkbench`) takes to recompute after a solve, and
how long its interactive steps take, on deterministic synthetic radial four-wire feeders. These are
CPU timings of the overview's own logic, not of the solver or the result transfer.

```sh
cd packages/svelte
DIAGRAM_SCALE=100000 DIAGRAM_SCALE_OUT=../../evidence/diagram/overview-scale-100000.json \
  npx vitest run tests/diagram-scale.test.ts
```

Without `DIAGRAM_SCALE` the test only checks correctness on 10,000 buses; timings never gate CI.
Each file records five measured runs after one warm-up, with the median and maximum per step.

Recorded on a MacBook Pro (Apple M4 Max, 48 GB) with Node 24:

| Buses   | Per-solve overview (median) | Largest step                     |
| ------- | --------------------------- | -------------------------------- |
| 10,000  | 26 ms                       | `snapshotFromMcPf`, 23 ms        |
| 100,000 | 219 ms                      | `snapshotFromMcPf`, about 190 ms |

The per-solve figure is snapshot, issue ranking and summary. `graphInputs`, the display tree and
the search index depend only on the network and are built once per case.

At 100,000 buses the result is 400,000 terminals and 800,000 element ports. Indexing them costs
about 60 ms and building the bus and branch views about 130 ms; a live session must also serialise
and parse that full result first. The Studies view therefore refreshes the overview automatically
only up to 25,000 buses. Larger cases show an explicit _Analyse operating point_ action, so load
edits never stall the page. A compact columnar result transfer from the engine is the path to
automatic refresh at full scale.
