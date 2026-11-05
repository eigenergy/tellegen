# Operating-point overview at scale

How long the multiconductor Studies overview (`McWorkbench`) takes to recompute after a solve, and
how long its interactive steps take, on deterministic synthetic radial four-wire feeders. These are
CPU timings of the overview's own logic, not of the solver or the result transfer.

```sh
cd packages/svelte
DIAGRAM_SCALE=100000 DIAGRAM_SCALE_OUT=../../docs/evidence/diagram/overview-scale-100000.json \
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

# Substation overview layout

Quality and speed of the balanced-case schematic (`tellegen::tx_schematic`), with the metrics of
Birchfield and Overbye, "Techniques for drawing geographic one-line diagrams: substation spacing
and line routing" (IEEE Trans. Power Systems 33(6), 2018, §V). Each case is laid out with lines
straight between their boxes and with routing.

```sh
TX_EVAL_CASE=case_ACTIVSg200.m TX_EVAL_COORDS=ACTIVSg200-coords.csv \
  TX_EVAL_OUT=$PWD/docs/evidence/diagram/tx-layout-activsg200.json \
  cargo test --release -p tellegen --test tx_schematic_eval evaluate_case -- --ignored --nocapture
TX_EVAL_OUT=$PWD/docs/evidence/diagram/tx-layout-synthetic.json \
  cargo test --release -p tellegen --test tx_schematic_eval evaluate_synthetic -- --ignored --nocapture
```

`TX_EVAL_COORDS` is a `bus_i,Lat,Lon` CSV. For ACTIVSg200 it was derived from the substation table
of `ACTIVSg200.aux` (TAMU), joining each bus to its substation's coordinate. The synthetic grids
scatter substations over a 1,000 km square, a third of them around ten cities, with two voltage
levels each and lines to their next two neighbours in a space-filling order; their lines are
longer than real ones, which makes routing harder. Both tests are opt-in and never gate CI.

Recorded on a MacBook Pro (Apple M4 Max) with a native release build:

| Case | Buses | Layout, routed | Not moved | Max displacement | Overlapping boxes | Segments through other substations, straight → routed | Relative length, p90 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| ACTIVSg200 | 200 | 0.3 ms | 86 % | 10.1 km | 0 | 20 → 0 | 1.27 |
| synthetic | 4,000 | 14 ms | 93 % | 5.1 km | 0 | 843 → 32 | 1.10 |
| synthetic | 20,000 | 67 ms | 94 % | 2.3 km | 0 | 5,329 → 172 | 1.08 |
| synthetic | 70,000 | 0.33 s | 95 % | 1.3 km | 0 | 25,723 → 816 | 1.05 |

Placement never overlaps two substation boxes and moves few substations from their geographic
point, as in the paper's greedy method. Routing through the Delaunay channels, with a final step
around any box still in the way, removes nearly every line crossing another substation, at the
cost of about two more bends per line on the synthetic grids. Substation grouping itself takes
13 ms for 70,000 buses (ACTIVSg70k without coordinates).

## Without coordinates

Cases whose positions are synthetic are laid out by their connections instead
(`TxSeeding::Stress`): sparse stress (Ortmann, Klimenta and Brandes 2017) on the substation graph,
started from pivot MDS (Brandes and Pich 2006), then the same placement and routing. Without
`TX_EVAL_COORDS` the evaluation groups buses by transformers alone and seeds by stress
(`tx-layout-stress.json`, the TAMU ACTIVSg MATPOWER files):

| Case | Buses | Substations | Layout, routed | Overlapping boxes | Segments through other substations, straight → routed |
| --- | ---: | ---: | ---: | ---: | ---: |
| ACTIVSg2000 | 2,000 | 1,249 | 28 ms | 0 | 932 → 48 |
| ACTIVSg10k | 10,000 | 7,210 | 0.15 s | 0 | 5,063 → 756 |
| ACTIVSg70k | 70,000 | 54,587 | 1.2 s | 0 | 34,609 → 4,873 |

Stress takes most of the time at this scale (0.8 s of 1.2 s for ACTIVSg70k, 64 pivots and at most
60 passes); it replaces the browser's force placement, whose drawing of a meshed grid this size is
a tangle.
