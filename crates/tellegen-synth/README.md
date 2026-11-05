# tellegen-synth

Geo-referenced synthetic transmission grids for a map region, produced as
[PowerIO](https://crates.io/crates/powerio) networks and validated with the
`tellegen` engine.

Given a GeoJSON region, a seed and a handful of planning parameters, the
generator:

1. measures the region and its populated places (GeoNames cities5000,
   CC BY 4.0, https://www.geonames.org/, bundled as
   `data/populated_places.json`), sizes peak demand from each place's
   country (Ember's national demand via Our World in Data, CC BY 4.0,
   bundled as `data/national_demand.json`, the unlisted part spread over
   each country's area) or, in the United States, its state (EIA-861 sales,
   Census population and EIA-930 peak factors, public domain,
   `data/subnational_demand.json`), takes the voltage ladder and fuel mix of
   the dominant state or country (`data/voltage_ladders.json`,
   `data/subnational_fuel_mix.json` from EIA-860), and resolves the bus
   count;
2. sites buses on the most populous places plus population-weighted
   blue-noise fill, and allocates demand weighted loads;
3. wires each electrical island with a minimum spanning tree, a spanning
   tree over the highest voltage buses, a Gabriel proximity mesh and a few
   high-voltage loops, all from one Delaunay triangulation (O(n log n)). A
   ladder that reaches subtransmission (below 100 kV) is built level by
   level instead (`hierarchy`): sites per level, a meshed top level, and
   subtransmission pockets fed from step-downs, with tap junctions;
4. sites reserve-margin generation area by area (no bus collects more
   than its voltage class can evacuate), designs every corridor from cited line and
   transformer class tables with St. Clair loadability, and plans the
   network before validating it: corridor voltage by power and distance,
   circuits for the peak economic dispatch, an island angle spread inside
   the steady-state stability margin, synchronous condensers for remote
   pockets and regulated subtransmission secondaries, and shunt banks that
   balance reactive power at every bus;
5. validates with the engine: a DC optimal power flow and an AC power flow
   per island, plus a recorded calibration over a declared parameter set.

Every verdict in the `GenerationReport` is the engine's own: an infeasible
DC OPF is reported as infeasible, an AC power flow that does not converge is
reported as such, and calibration says `Unchanged` when no step improved its
declared objective. The module carries bus locations and branch routes, so
it renders on a map without a sidecar.

The library has no clock, no filesystem access and no platform randomness:
the same request and places table reproduce the same grid, byte for byte, on
the same build and platform. Native and WebAssembly runs agree in structure in
the cases tested, but their floating-point values differ in the last bits,
because the two platforms' math libraries round `ln`, `exp`, `sin` and `cos`
differently. Compare values across platforms with a tolerance, not by hash.

```rust
use std::sync::Arc;
use tellegen_synth::{generate, Places, Region, SynthRequest};

let table = br#"[{"n":"Accra","lo":-0.2187,"la":5.552,"p":2121000,"c":"GHA"}]"#;
let places = Arc::new(Places::parse(table).expect("places table"));
let request = SynthRequest::new(Region::bbox(-2.0, 5.0, 1.0, 8.0, "Gulf of Guinea"));
let (module, report) = generate(request, places, None).expect("generation");
let dc = report.dc_opf.expect("the DC OPF stage ran");
println!("{} buses, DC OPF {:?}", module.value().buses().len(), dc.status);
```

`Generator` exposes the same pipeline one stage at a time (`step`), with a
map preview between stages, for hosts that show progress or cancel.

With `source: "osm"` the buses, lines and plants come from an Overpass
document of OpenStreetMap `power=*` features (ODbL) instead, and the same
planning and validation run on the mapped network. How close California comes
to the California Test System in both modes is measured in
`docs/evidence/synth/cats/`.

Features: `schema` derives JSON Schema for the generated TypeScript
types; `embedded-places` bundles `data/populated_places.json` for the
CLI and offline tests.

## Related

- [MapToGrid](https://maptogrid.com) runs this generator in the browser
  through `@tellegen/engine/synth`. The `tellegen generate` command runs it
  from the command line with the same request and seed, and writes the module,
  the report, a GeoJSON layout and any PowerIO case format.
- Every module this crate writes is a PowerIO network, so PowerIO's Rust,
  Python, Julia and C interfaces read it and convert it to MATPOWER, PSS/E,
  pandapower, PowerModels, XIIDM and the other formats PowerIO writes.
- [`powerio-synth`](https://crates.io/crates/powerio-synth) generates abstract
  topologies (tree, lattice, PEGASE-like mesh) without loads, generators or
  geography, for format and matrix stress tests. This crate generates
  geo-referenced grids with demand, generation and voltage levels, validated
  by the tellegen engine.
