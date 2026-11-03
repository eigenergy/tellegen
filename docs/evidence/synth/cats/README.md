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

The gridded correlations use the cells where either network has a value.

**Pass rule.** Every critical check, plus at least 80% of the scored metrics, in every run.

## Results

- `baseline/`: the generator before any California-specific data or tuning (the US
  national ladder and demand). 7 of 47 metrics pass at every seed.
