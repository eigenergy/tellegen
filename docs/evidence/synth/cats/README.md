# California against CATS

How close a generated California grid comes to the
[California Test System](https://github.com/WISPO-POP/CATS-CaliforniaTestSystem) (CATS;
Taylor et al., *IEEE Transactions on Energy Markets, Policy and Regulation* 2(1), 2024,
doi:10.1109/TEMPR.2023.3338568), a geographically accurate synthetic network built on
California's real lines, substations and plants. Both networks are measured by the same
code (`metrics.py`), and `compare.py` scores the generated one against CATS with fixed
tolerances. The generator never reads CATS. It is a yardstick only.

```sh
scripts/stage-data.sh ~/datasets data          # CATS into data/CATS (gitignored)
cargo build --release -p tellegen-cli
uv run --script docs/evidence/synth/cats/compare.py run \
  --cats data/CATS --tellegen target/release/tellegen --work /tmp/cats-synthetic
# OpenStreetMap mode, from an Overpass answer for the same region:
uv run --script docs/evidence/synth/cats/compare.py run --mode osm --osm california-osm.json \
  --cats data/CATS --tellegen target/release/tellegen --work /tmp/cats-osm
```

`uv` reads the dependencies (numpy, scipy) from the scripts' inline metadata. Each run
writes `scorecard.md` and `scorecard.json` into `--work` and prints one line: the verdict,
the mean and worst score over the seeds (the sum of each metric's error in units of its
tolerance, capped at 3, plus 10 per failed critical check), and the metrics passed.
`compare.py reference` rewrites `cats_reference.json`, the CATS metrics summary committed
here. CATS data itself (BSD-3-Clause) is not copied into the repository.

## The region

`california.json` is California (US-CA) from Natural Earth v5.1.2 1:50m admin-1, geometry
copied unchanged. That is the region MapToGrid sends when California is selected, with the
app's default request. Seeds 1, 7 and 2026 (2026 is the app's default).

## How both networks are measured

- **Main component.** The largest connected component.
- **Series reduction.** Degree-2 buses with no load and no machine whose two branches are
  lines at one voltage are contracted. CATS keeps a bus at many digitising vertices; most
  of its 5,699 "added nodes" are degree-3 taps, which stay. CATS goes from 8,870 buses to
  8,347.
- **Sites.** Buses within 300 m of one another; one substation with several voltage levels
  is one site.
- **Voltage classes.** 66, 115, 230 and 500 kV, the nearest by log distance (69 → 66,
  138 → 115, 345 → 500).
- **Bands** (Birchfield et al. 2017, Table 5, plus subtransmission). lv below 100 kV, mv
  100–200 kV, hv above.
- **DC OPF.** One LP for both networks, in flow form with linear costs: the generators'
  marginal cost at half output, and every branch's rate A.
- **Siting.** MapToGrid's measure of California (tellegen#305): buses at sites against the
  people of the bundled places table's California rows (subdivision `US-CA`), in 0.5° cells.
  A generated grid's tap junctions and CATS's added nodes (line junctions, most of them in
  cities) are left out, so CATS counts its 3,171 substation buses. The metrics are the
  share of those buses in cells whose places hold fewer than 20,000 people, the share in the
  ten most populous cells, and buses per 1,000 km² in three boxes: the Los Angeles basin
  (118.7–117.0° W, 33.6–34.4° N), the Mojave Desert (117.5–115.5° W, 34.6–35.6° N) and
  Modoc and Lassen counties (121.5–120.0° W, 40.5–42.0° N), with the ratio of the first to
  the last. A generated grid sites more buses than CATS has substations (about 4,900
  against 3,171), so its box densities run above CATS's even where the spread matches;
  the shares and the ratio do not depend on the count.

## Scorecard

Tolerances are per mode: S for Synthetic, O for OpenStreetMap, which starts from real
geography and is held tighter there. A metric CATS has but a generated grid lacks (a
voltage class it does not model, say) counts as a failure.

| Group | Metric | S | O |
| --- | --- | --- | --- |
| Critical | Main component holds ≥ 95% of load; DC OPF optimal (engine and harness); AC power flow converged with every bus in 0.90–1.10 pu | must | must |
| Scale | Peak load | ±15% | ±15% |
| | Nameplate excluding condensers and imports | ±20% | ±20% |
| | Buses after series reduction; sites | ×0.70–1.40 | ×0.75–1.33 |
| | Bus share per class | ±0.06 | ±0.06 |
| | Circuit-km per class | ×0.67–1.5 | ×0.8–1.25 |
| | Transformers per site | ×0.67–1.5 | ×0.67–1.5 |
| | Generators excluding condensers | ×0.5–2 | ×0.5–2 |
| | Fuel capacity mix, total variation distance | ≤ 0.15 | ≤ 0.15 |
| Topology | Mean degree | ±12% | ±12% |
| | Degree distribution, L1 distance | ≤ 0.25 | ≤ 0.20 |
| | Lines per site, per band | ±15% | ±15% |
| | Line length / minimum spanning tree, per band | ±25% | ±20% |
| | Share of lines on the Delaunay triangulation, per band | ±0.10 | ±0.10 |
| | Mean clustering coefficient | ±0.05 | ±0.05 |
| | Mean hop distance | ×0.67–1.5 | ×0.67–1.5 |
| Parameters | Share of lines whose x per km, X/R and rating fall inside CATS's 10th–90th percentile at each class (the 70% rule of Birchfield et al. 2017) | ≥ 0.70 | ≥ 0.70 |
| | Share of transformers whose reactance on their own base falls inside CATS's 10th–90th percentile | ≥ 0.80 | ≥ 0.80 |
| Operation | DC OPF branch loading, median and 90th percentile | ±0.15 | ±0.15 |
| | DC OPF binding fraction | ±0.05 | ±0.05 |
| Geography | Mean nearest-site distance, both ways (km) | ≤ 20 | ≤ 5 |
| | Spearman correlation of line-km per 25 km cell | ≥ 0.60 | ≥ 0.85 |
| | Spearman correlation of load per 50 km cell | ≥ 0.70 | ≥ 0.70 |
| Siting | Share of sited buses in cells under 20,000 people | ±0.07 | ±0.05 |
| | Share of sited buses in the ten most populous cells | ±0.10 | ±0.08 |
| | Sited buses per 1,000 km², per box | ×0.5–2 | ×0.5–2 |
| | Density ratio, Los Angeles basin over Modoc and Lassen | ×0.33–3 | ×0.5–2 |

The gridded correlations use the cells where either network has a value.

**Pass rule.** Every critical check, plus at least 80% of the scored metrics, in every run.

### What the map itself holds (OpenStreetMap mode)

An OpenStreetMap grid can only be as close to CATS as the map is. `metrics.measure_map`
measures the Overpass document itself inside the region, with no engine involved:

- nameplate and fuel mix of the `power=plant` features that tag their output (untagged
  plants are counted, not estimated, so this is a floor);
- every `power=substation`, a ceiling on sites;
- the voltage levels substations tag at 56 kV and above, as bus shares per class;
- circuit-km per class from the lines that tag a voltage, using `circuits` (else
  `cables / 3`, else 1) over their length inside the region;
- line-km per 25 km cell against CATS, gridded as for the generated grid.

A metric whose map value already misses the tolerance is **map-limited**. The map sets it,
not the engine, so it appears in the scorecard's Map column but is left out of the pass
share and the score. A floor limits a metric only when it lies above the tolerance, and a
ceiling only when it lies below. Plants and substations are tested against the exact
outline; line vertices against the outline rasterised at 0.02°.

## Results

All runs use the request in `california.json` with the engine in this repository.

- `baseline/`: the generator before any California-specific data or tuning (the US
  national ladder and demand). 7 of 47 metrics pass at every seed.
- `synthetic/`: **passes at every seed**, 41, 42 and 42 of 47 at seeds 1, 7 and 2026, with
  every critical check met (AC voltages 0.961–1.051 pu). It fails at every seed on
  circuit-km at 66 and 115 kV (1.6–1.9 × CATS), the fuel mix (TVD 0.22: CATS's 2019 fleet
  against the EIA-860 2025 shares the engine uses for California), and lines per site in the
  lv band. Seeds 1 and 2026 also miss the DC
  loading median, and seeds 1 and 7 the line-density correlation.
- `osm/`: **passes**, 34 of the 40 metrics the map supports, with every critical check met
  (DC OPF optimal; AC voltages 0.961–1.066 pu). Seven metrics are map-limited (below). It
  fails on buses after series reduction (3,314 against at least 6,260), sites (2,834
  against at least 3,749, where the map has 3,948 substations of any kind), transformers per
  site (0.38 against at most 0.20), mean degree (2.70 against at most 2.69), the degree
  distribution (L1 0.69 against 0.20) and line density (Spearman 0.83 against 0.85, where
  the map itself reaches 0.86). Over all 47 metrics, map-limited ones included, it passes
  34 (72%).

The OpenStreetMap document is MapToGrid's Overpass server's answer to the engine's query for
California (`tellegen synth-overpass-queries`), `osm_base` 2026-10-26T01:00:03Z,
40,212 elements, SHA-256
`74cf4a8bf4f4835ecf8290dc30502a006fc6961eff91a89a38568eeafa1e0b1b`. It is not committed.

The seven map-limited metrics, with what the map holds:

| Metric | CATS | Map | OSM grid |
| --- | ---: | ---: | ---: |
| Nameplate (GW) | 68.0 | at least 102.4 (1,194 plants tag no output) | 118.6 |
| Fuel mix TVD | – | 0.18 | 0.18 |
| Bus share at 66 kV | 0.651 | 0.451 | 0.422 |
| Bus share at 115 kV | 0.225 | 0.298 | 0.291 |
| Bus share at 230 kV | 0.113 | 0.224 | 0.254 |
| Circuit-km at 66 kV | 20,391 | 11,454 | 13,871 |
| Circuit-km at 230 kV | 13,352 | 21,979 | 23,936 |

These are differences of data, not of the design:

- **Vintage.** CATS's fleet is that of 2019. The map's is that of 2025: its tagged plants
  hold 11.5 GW of batteries against CATS's 2.1 GW of storage, and 26.0 GW of solar against
  11.6 GW.
- **Coverage.** The map holds about 11,500 circuit-km in the 66 kV class against CATS's
  20,400, and 1,750 of its 3,948 substations tag a voltage of 56 kV or more.
- **Modelling convention.** CATS models 1,015 of its 1,133 230 kV corridors as one branch,
  while most of California's 230 kV lines are double-circuit and the map tags them so. CATS
  also T-taps its substations through junction buses: 53% of its reduced buses have degree
  3, where mapped lines loop into substations (degree 2). This second convention is part of
  the degree failures above, which stay scored.

`climb-log.md` records the hill climb that set the tuned constants. The country sweeps one
directory up are the hold-out the climb never sees. Every change to the engine was checked
against them: on the final engine, DC OPF is optimal in 54 of 54 and 51 of 51 runs, and the
AC power flow converges inside 0.9–1.1 pu in 53 of 54 and 47 of 51.
