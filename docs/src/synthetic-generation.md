# Synthetic Grid Generation

`tellegen-synth` generates a geo-referenced transmission grid for a region
drawn on a map, validates it with the engine's DC optimal power flow and AC
power flow, and reports every verdict as the engine returned it. The result
is a PowerIO `BalancedNetwork` module whose buses carry locations and whose
branches carry routes, plus a `GenerationReport`. The crate is part of the
one wasm module (feature `synth`), so the whole pipeline runs in the
browser; the CLI runs the same code natively.

The grids are synthetic. Topology, impedances, ratings, loads and generation
are modelled from the region's geography and population with the cited
tables below, so a generated grid resembles a transmission system without
describing any real one. OpenStreetMap mode maps real substations, lines and
plants, but its electrical parameters and loads stay modelled.

A region is GeoJSON `Polygon` or `MultiPolygon` geometry in longitude and
latitude, or a bounding box. A region that crosses ±180 must arrive split at
the seam, as RFC 7946 asks, or as a bounding box whose east edge is above
180; a ring edge that jumps more than 180° of longitude is refused, except
along a pole. Request values are bounded well outside any real system:
voltage levels 1–1500 kV, `demand.mw` up to 10⁷, `demand.per_capita_kw` up
to 100 and `demand.peak_factor` from 1 to 10.

Each generated bus, branch, load, generator, shunt and HVDC row keeps its
annotations (kind, place, population weight, tier, island, source, corridor
and circuit) under `extras.tellegen_synth`, and the module is named
`tellegen_synth_` followed by the first 16 hex digits of the request hash.

## Stages

A session runs one bounded stage per call. The calibration stage runs once
per iteration.

| Stage | Work |
| --- | --- |
| `context` | parse and measure the region (spherical area, landmasses, island groups, keeping the most populous group when far islands are cut); resolve peak demand from the national demand table, the voltage ladder, the bus count and the fuel mix |
| `site_buses` | anchor buses on the most populous places, fill the rest with blue noise; allocate demand-weighted loads, holding every bus at or below the top level's load cap |
| `topology` | per island, a minimum spanning tree, a tree over the highest-voltage buses, a Gabriel mesh up to the branch factor, and high-voltage loops, all from one Delaunay triangulation |
| `electrify` | site generation; design every corridor; run the corridor plan and the reactive plan; build the PowerIO network |
| `dc_opf` | the engine's DC OPF per island |
| `calibrate` | one recorded calibration step per call |
| `ac_pf` | the engine's AC power flow per island at the DC OPF dispatch |
| `finalize` | wrap the network in a module with its producer and history |

Every stage up to `electrify` is O(n log n). The library reads no clock, no
filesystem and no platform randomness. Each stage draws from its own ChaCha8
stream seeded from the request seed, so the same request and places table
give the same module, byte for byte, on the same build and platform. Native
and wasm runs agree in structure in the cases tested, but floating-point
values differ in the last bits because the platform math libraries round
`ln`, `exp`, `sin` and `cos` differently; compare them with a tolerance, not
by module hash.

## Demand and bus count

- **National demand.** By default each country's peak is its annual
  demand from Ember's Yearly Electricity Data (via Our World in Data's
  energy dataset, CC BY 4.0) as an average load, times a peak factor of
  1.3. The table is `crates/tellegen-synth/data/national_demand.json`,
  derived by `scripts/derive-national-demand.py` from
  `owid-energy-data.csv`; 196 of 212 countries report 2024. A place whose
  country has no entry draws the world average per person and raises
  `demand_fallback`. `demand.per_capita_kw` sets one average per person for
  every place instead, and a `total_mw` demand states the total.
- **Listed and unlisted demand.** The places table lists a country's
  cities, not all of its people: from 5% of Ethiopia's population to 72% of
  Japan's. So a country's peak splits in two:
  - The share its listed places explain (their population over the national
    population, `listed_share`) follows those places, by population.
  - The unlisted rest is spread over the country's area. A region takes it
    in proportion to the country area it covers (`area_share`; each part of
    the region counts to the country of its nearest place), but never beyond
    its share of the country's listed population, since a drawn box may be
    sea or desert; covering 95% of the area takes all of it.

  A region covering a whole country reproduces its national peak. A box
  holding a country's few listed places takes only the part of the
  country it covers. Leaving far islands out removes their places, and
  with them their share by population, not by area. Country areas come from
  Natural Earth's 1:50m admin-0 polygons, measured with the generator's own
  spherical area (`data/country_areas.json`, derived by
  `scripts/derive-country-areas.py`). The report lists the countries behind
  the demand with these shares (`context.demand_by_country`), and both
  tables' hashes (`provenance.demand_table`).
- **Peak factor.** Annual load factors run from 0.58 to 0.77 in the
  systems surveyed (OCCTO, ENTSO-E, PJM, ERCOT, NEA, India's Ministry of
  Power), a peak factor of 1.3 to 1.72. Ember's demand counts losses and
  self-supplied use that operators' peak series leave out, so the factor
  applied to it is the low end, 1.3. It is a modelling choice within that
  range.
- **Places at the coast.** Generalised country outlines leave out harbours
  and estuaries: at 1:50m, New York lies 1.9 km outside the United States
  and Lisbon 1.6 km outside Portugal. A place up to 15 km outside the region
  joins it, moved onto its edge, when the region holds at least half of
  that country's place population counting such places
  (`context.places_snapped`). A drawn box around one city never snaps the
  cities beyond its edges.
- **Bus count.** Without `target_bus_count`, the count follows the region's
  area and density and is then moved into the range that keeps peak load
  per bus between 30 and 150 MW. Those are the scales of transmission
  models down to 110–220 kV (PEGASE, RTE) and of substation-level models at
  220 kV and above (PyPSA-Eur, SciGRID). `limits.max_bus_count` (3000)
  still caps it, and `bus_count_capped` says by how much.
- **Load cap.** No bus carries more than the load cap of the highest
  modelled level (7 GW at 500 kV, 2.9 GW at 400 kV). The excess of a metropolitan bus moves
  to the nearest buses of its island with room, and
  `buses_load_capped` and `load_spilled_mw` count it.

## Voltage levels

- **National ladders.** A request that names no `voltage_levels_kv` (the
  default) is modelled at the AC transmission levels of its region's
  dominant country by demand, from `data/voltage_ladders.json`, a table of
  national ladders with their sources: for example 380/220 kV in Germany,
  400/275 kV in Great Britain, 500/345/230 kV in the United States and
  500/275 kV in Japan. `context.voltage_basis` says `country`,
  `context.voltage_country` names the country, and `provenance.voltage_table`
  gives the table's version and hash.
- **Higher levels.** The table also lists any higher AC level a country has
  in service: 765 kV in the United States, India, Brazil and South Africa,
  750 kV in Russia, 1000 kV in China and 500 kV in eastern Australia
  (`context.promotion_levels_kv`). Only these may be added above the ladder
  by the corridor plan. A country with none, such as Germany, France, Great
  Britain, Spain, Portugal or Japan, keeps its ceiling: a corridor short of
  capacity at four circuits stays capped and the plan reports
  `planning_incomplete`.
- **Fallback.** A country without an entry takes the ENTSO-E members'
  400/220 kV when it is a member, else IEC 60038's standard 400/220 kV, with
  no higher level, and raises `voltage_fallback` (`context.voltage_basis`
  `fallback`).
- **Stated levels.** A request that names its levels is modelled at them,
  and the plan may add 765 kV above them, as before (`voltage_basis`
  `request`).
- **Line parameters.** A level between the line classes takes the nearest
  class's conductors at its own voltage (below), so 380 and 400 kV use the
  345 kV class scaled by 1.10 and 1.16, and 1000 kV the 765 kV class scaled
  by 1.31.

## Parameters

Every constant lives in `crates/tellegen-synth/src/electrical/params.rs`
with a `source:` comment, apart from the demand and siting constants, which
sit beside the code that uses them.

Line classes. Representative ACSR bundles from Glover, Overbye and Sarma,
*Power System Analysis and Design*; SIL from Kundur, *Power System Stability
and Control*, Table 6.1:

| Class kV | r Ω/km | x Ω/km | b µS/km | thermal MVA | SIL MW |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 138 | 0.11 | 0.48 | 3.4 | 180 | 50 |
| 230 | 0.06 | 0.45 | 3.8 | 500 | 140 |
| 345 | 0.03 | 0.35 | 4.8 | 1200 | 400 |
| 500 | 0.02 | 0.30 | 5.6 | 2500 | 900 |
| 765 | 0.012 | 0.26 | 6.5 | 4500 | 2200 |

- **Voltage levels between classes.** A requested level such as 220 kV or
  34.5 kV takes the nearest class's conductors at its own voltage. Its
  thermal rating scales with the voltage (same ampacity, `S = √3 V I`), and
  its SIL with the square (same surge impedance, `SIL = V² / Zc`).
- **Ratings.** A line's rating is the lesser of its thermal rating and SIL
  times the St. Clair loadability. The loadability curve follows Dunlop,
  Gutman and Marchenko (1979): 3.0 SIL up to 80 km, 1.0 at 480 km. A rating
  is never raised above what the conductor and length allow.
- **Transformers.** Impedance follows the size class: 8, 10 or 12% on the
  unit's own base, after IEEE C57.12.10 and Grainger and Stevenson. Banks
  are sized for the larger of local load and local generation, with 20%
  headroom. Buses sit at their own base kV, so the off-nominal tap is 1.0;
  the winding ratio is recorded in the branch extras.
- **Generation.** Fuel unit sizes and costs come from EIA-860 and the EIA
  Electric Power Monthly. Installed capacity is (1 + reserve margin) × peak
  load, with an 18% default margin. No bus collects more generation than
  its class can evacuate: 400 MW at 115–161 kV, 1.2 GW at 230 kV, 2.7 GW at
  345 kV and 4 GW at 500 kV, following the interconnection voltages of the
  largest U.S. plants.
- **Regional adequacy.** An island's units are grouped into adequacy
  areas of about 10 GW of the load nearest them, the scale of the Lower
  48's 66 balancing authorities. Each area holds the reserve-margin capacity
  of its own load, shared among its units by their drawn sizes, so the
  network carries balancing transfers between neighbours rather than one
  side of a country's demand to the other. A share above a bus cap spills
  onto the area's other units, then onto the area's next ranked bus.

## Planning before validation

A generated grid is planned so that it is feasible by construction, not
repaired after a solver refuses it. No number in a report is adjusted after
a solve.

- **Planning case.** The planning case is peak load under the least-cost
  dispatch of the generators (equal incremental cost) on a lossless DC power
  flow: the grounded Laplacian, solved by Jacobi-preconditioned conjugate
  gradients.
- **Voltage class by power and distance.** A corridor that would need more
  than two circuits at its voltage raises its ends to the next level. Above
  the highest modelled level the plan may add only a higher level in service
  (`added_level_kv`, warning `voltage_level_added`).
- **Circuit cap.** A corridor holds at most four circuits: double and
  quadruple circuit lines are common in Germany, India, Japan and China, and
  in the OpenStreetMap-based European grid no line at 220 kV and above
  carries more than four per voltage level. A corridor that would need more
  raises its ends instead. One that still needs more at the highest level
  available is counted in `capped_corridors` and reported.
- **Capacity.** Circuits are added until no circuit runs above 90% of its
  rating in the planning case; this is Garver's flow-driven expansion under
  the NERC TPL-001 base-case requirement. A transformer corridor first takes
  a larger bank, up to the rating of its line.
- **Stability margin.** Each line within its own St. Clair rating can still
  add up to too wide an angle along a chain. So the plan aims to hold every
  island's angle spread to asin(0.7) ≈ 44.4°, the St. Clair curve's 30%
  steady-state margin, reinforcing the corridors a first-order spread
  sensitivity ranks highest. It raises buses within the requested levels
  until 35% of them sit at the highest one, and only then onto 765 kV. It
  adds circuits only up to 1.6 branches per bus, about a tenth above the
  densest North American systems (1.2–1.5, Birchfield et al. 2017). The
  last quarter of the planning rounds only adds capacity. A spread still too
  wide is reported as `planning_incomplete`, not met by a network denser
  than real ones.
- **Branch factor.** Circuits the requested branch factor still leaves go
  where the planned loading is heaviest. The plan exceeds the factor only
  when capacity or the margin needs more, and then raises the
  `branch_target_exceeded` warning.
- **Reactive plan.** The reactive plan holds a flat profile at the
  generator set point (1.02 pu). It solves the AC active-power equations for
  the angles at the planning dispatch (fast decoupled P–θ, Stott and Alsaç
  1974), then installs a capacitor bank or a shunt reactor wherever a bus
  needs more reactive power than half its machines' capability provides.
  Reactive power does not travel far (Kundur §11.2), so it is supplied where
  it is used. This plan covers line charging, series absorption and the
  share resistance moves to the receiving end.
- **Voltage support.** A loaded bus farther than 480 km of corridor from
  any generator gets a synchronous condenser, farthest first: no active
  power, and a reactive range of 0.5 MVAr per MW of the load nearest it.
  480 km is where the St. Clair curve falls to 1.0 SIL and voltage-drop and
  stability limits replace the thermal one. Without one, a remote pocket fed
  over long lines has nothing to hold its voltage, and its AC power flow can
  stall while the lines are well inside their ratings. Condensers are
  flagged in the generator annotations (`condenser`) and counted in the
  `electrify` stage (`condensers`); a mapped OSM network gets none.

The plan is recorded in `GenerationReport.planning`: rounds, promoted
buses, the added level, the most circuits on one corridor
(`max_parallel_circuits`) against the cap (`circuit_cap`), capped
corridors, capacity, stiffening and filler circuits, enlarged banks, angle
spread, and banks and MVAr installed. Each circuit is its own branch row;
its annotation in `GenerationReport.elements` gives the corridor's circuit
count (`circuits`) and the branch's 1-based place among them
(`circuit_index`).

## Validation and calibration

- **Islands.** The DC OPF and the AC power flow run per electrical island,
  each with its own reference bus.
- **HVDC.** Links are replaced by fixed transfers on a validation copy,
  because the engine's models do not read HVDC rows.
- **Failures.** An infeasible DC OPF is reported as infeasible with the
  engine's message; nothing is shed. Load shedding is off in the portable DC
  OPF, so unserved energy is 0 when optimal and absent otherwise. An AC
  power flow that does not converge is reported as such.
- **Calibration.** Calibration is a recorded search over declared parameters
  against a declared objective:
  - parameters: rating scale per class and capacity scale per fuel within
    [0.8, 1.5], rating notches on binding branches, and load scale only on
    request;
  - objective: DC infeasibility, worst loading above target,
    under-utilisation, binding fraction, reserve margin, plus the structural
    terms the parameters cannot move.
- **Calibration steps.** Every candidate is an exact DC OPF re-solve; a step
  that does not improve the objective is rejected and undone. The status
  says `Unchanged` when nothing improved, and the final objective is the
  measured one.

## OpenStreetMap mode

With `source: "osm"` and an Overpass document (one, or the two halves of an
antimeridian-crossing region), the grid is built as follows:

1. Substation sites within 500 m cluster, and each voltage level at or
   above 110 kV gets a bus, with a transformer between consecutive levels.
2. Fragmented ways stitch where exactly two ends meet away from a station.
3. Untagged voltages are inferred from the endpoints, and line ends snap to
   the bus of their voltage within 2 km. Parallel ways aggregate into one
   corridor.
4. DC ways between converters become HVDC rows.
5. Plants snap within 10 km; makeup units cover a shortfall against the
   reserve margin.
6. The island policy applies: `connect` (default), `cut` or `keep`.

`limits.max_bus_count` (3000) caps this mode too. When the mapped substation
levels and converters make more buses than that, the lowest substation level
is left out, one at a time, until they fit, and its lines with it;
`osm_bus_count_capped` names the levels kept. A region whose highest level
alone exceeds the cap is refused as `invalid_request`: choose a smaller region
or raise the cap.

Mapped circuits are real, so the corridor plan only measures them; the
reactive plan still applies. A way's circuit count, and a corridor's, is
capped at 12: a larger `circuits` or `cables` tag is a tagging error, and each
circuit becomes a branch row. A network too thin to use is the typed
`osm_insufficient` refusal. The caller decides whether to rerun
synthetically; the generator never switches on its own. The Overpass QL
comes from the engine (`syntheticOverpassQueries`), so its text exists once.

An OpenStreetMap-mode grid contains data derived from OpenStreetMap, ©
OpenStreetMap contributors, available under the Open Database License
(ODbL) 1.0. Check the ODbL's attribution and share-alike terms before
redistributing such a module or its exports.

## Using it

In the browser (`@tellegen/engine`):

```ts
import { createSynthSession } from "@tellegen/engine";

const session = await createSynthSession({ region: { geometry, label: "Portugal" } });
const report = await session.run((step, preview) => {
  // step.stage, step.progress; preview holds typed arrays for binary map layers
});
const moduleJson = await session.takeModule(); // pio-ir with Bus.location and Branch.route
const ingest = await session.ingest();         // the drop payload of that module
session.free();
```

- **Host.** Sessions run on a dedicated worker. Cancelling one terminates
  only that worker, so interactive studies are untouched.
- **Places table.** The Natural Earth table (public domain) ships in the
  package and loads once per worker. It is derived from Natural Earth
  v5.1.2's 1:10m populated places by `scripts/derive-populated-places.py`,
  names that release and its file's SHA-256, and the report repeats both
  (`provenance.places`). A host may load its own bare array of
  `{n, lo, la, p, c}` rows instead; its provenance then names no source. The
  national demand table is compiled into the module.
- **Types.** Request and report types are generated from Rust under
  `@tellegen/engine/synth`.
- **Strict requests.** An unknown key at any level of the request, such as
  a misspelt `limits` or `islands` option, is refused as `invalid_request`
  rather than ignored. `limits.max_area_km2` must be positive and finite,
  and `limits.max_branch_factor` finite and at least 1.
- **Coordinates.** The ingest payload has `coords_kind: "synthetic"` and
  `coords_method: "generated"`: the buses are sites chosen on real
  geography, not a topology layout, and the case panel says so.

From the command line:

```sh
tellegen generate --request request.json --out run/ --export matpower,psse35 --gridfm
tellegen stats --case case.m          # or a PowerIO module on stdin
```

Exports go through the engine's `Study.export`: MATPOWER, PSS/E (v33, v34,
v35, RAWX), PowerWorld, PSLF, PowerModels JSON, pandapower JSON and the IIDM
formats. The GridFM graphkit JSON writer belongs to `tellegen-synth`, because
PowerIO's `gridfm` format is the Parquet dataset layout.

## Limits

- Requests are capped at 3000 buses (`limits.max_bus_count`), which the
  automatic count reaches for the largest countries. OpenStreetMap mode
  keeps to the same cap by leaving out its lowest voltage levels. China at
  3000 buses takes about 1.0–1.2 s natively
  and 1.5 s in wasm (Node, Apple M5 Pro), end to end with both validations;
  the United States takes 1.4–1.5 s and 2.0 s, about 0.9 s of it an AC power
  flow that does not converge over five starts. A 500-bus country takes
  under 0.1 s. `docs/evidence/synth` records the measurements.
- Whether the AC power flow converges inside the band depends on the seed
  for several countries. `docs/evidence/synth` sweeps three seeds on two
  outlines (Natural Earth 1:50m and the MapToGrid app's 1:110m layer):
  91% and 92% of the runs pass AC, every run passes DC, and the United
  States fails at every seed.
- Clustering in generated grids runs higher than in the ACTIVSg200 and
  GOC500 references, because the Gabriel mesh is rich in triangles. Their
  highest nodal degree is also lower, since the mesh caps it at 8.
  `docs/evidence/synth` measures both against the references.
- Continental regions span several thousand kilometres as one AC island.
  Their planning-case angle spread stays well above 44° within the branch
  budget, so the plan reports `planning_incomplete`. Real systems of that
  size split into synchronous areas joined by HVDC, which the generator does
  not model yet; very long corridors, such as across the Nullarbor or
  between Indonesian islands, stay AC.
- The engine's AC power flow starts from a flat profile. On long, weakly
  coupled systems it can fail to converge, or settle on a second solution
  outside the voltage band, even when the planned operating point is valid.
  A warm start from the stored case state, once the engine offers it,
  removes this.
