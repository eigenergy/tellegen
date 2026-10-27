//! OpenStreetMap mode on a hand-made Overpass answer: substations and their
//! voltage levels, stitching, voltage inference, dropped distribution and
//! dangling lines, HVDC links, plants and makeup generation, clipping, the
//! island policies, and the typed insufficient-data refusal.

mod common;

use tellegen_synth::{
    generate, AcPfOutcome, DemandSpec, OsmIslands, OsmOptions, Region, SolveOutcome, Source,
    SynthError, SynthRequest,
};

const FIXTURE: &str = include_str!("fixtures/overpass_small.json");

fn request(islands: OsmIslands) -> SynthRequest {
    let mut req = SynthRequest::new(Region::bbox(8.0, 49.6, 9.5, 50.6, "Main valley"));
    req.source = Source::Osm;
    req.demand = DemandSpec::TotalMw { mw: 1500.0 };
    req.osm = Some(OsmOptions {
        islands,
        ..OsmOptions::default()
    });
    req
}

fn count(report: &tellegen_synth::GenerationReport, key: &str) -> u64 {
    report.provenance.osm.as_ref().unwrap().counts[key]
}

#[test]
fn the_fixture_maps_to_a_grid_that_solves() {
    let (module, report) = generate(
        request(OsmIslands::Connect),
        common::places(),
        Some(FIXTURE.to_owned()),
    )
    .unwrap();
    let net = module.value();
    // Sites A and C have two levels each, B is mapped twice and clusters, D
    // and E have one, plus two converters; the 20 kV site and the site outside
    // the region are gone.
    assert_eq!(
        net.buses().len(),
        9,
        "{:?}",
        net.buses().iter().map(|b| &b.name).collect::<Vec<_>>()
    );
    assert_eq!(count(&report, "substation_sites"), 5);
    assert_eq!(count(&report, "converters"), 2);
    assert_eq!(
        count(&report, "clipped"),
        2,
        "one substation and one plant lie outside"
    );
    assert_eq!(count(&report, "lines_stitched"), 1, "the split 380 kV way");
    assert_eq!(
        count(&report, "voltage_inferred"),
        1,
        "the untagged way takes 110 kV"
    );
    assert_eq!(
        count(&report, "lines_dropped"),
        2,
        "the 20 kV line and the dangling way"
    );
    assert_eq!(count(&report, "transformers"), 2, "one per two-level site");
    assert_eq!(count(&report, "hvdc_links"), 1);
    assert_eq!(
        count(&report, "interconnects"),
        2,
        "each converter joins the AC network"
    );
    assert!(net
        .buses()
        .iter()
        .all(|b| b.location.unwrap().kind == Some(powerio::geo::CoordsKind::Source)));
    assert!(net.buses().iter().all(|b| b.base_kv >= 110.0));
    assert_eq!(net.hvdc().len(), 1);

    // The parallel 110 kV line and cable between A and D aggregate: 1 + 6/3 circuits.
    let elements = report.elements.as_ref().unwrap();
    let ad: Vec<_> = elements
        .branches
        .iter()
        .filter(|b| b.osm_ids.contains(&305))
        .collect();
    assert_eq!(ad.len(), 3);
    assert!(ad.iter().all(|b| b.osm_ids.contains(&311)));
    assert!(ad.iter().all(|b| b.circuits == 3));
    let mut ad_index: Vec<u32> = ad.iter().map(|b| b.circuit_index).collect();
    ad_index.sort_unstable();
    assert_eq!(ad_index, [1, 2, 3]);
    assert!(common::check_circuit_annotations(net, &report) > 0);
    let hvdc: Vec<_> = elements.branches.iter().filter(|b| b.is_hvdc).collect();
    assert_eq!(hvdc.len(), 1);
    assert_eq!((hvdc[0].circuits, hvdc[0].circuit_index), (1, 1));

    // Mapped plants: 2x600 MW coal at A (its two mapped 600 MW units are the
    // same capacity), 300 MW wind at B, and an untagged solar plant at E
    // (estimated) with an untagged solar generator beside it (left out); makeup
    // covers the rest of the reserve margin.
    assert_eq!(count(&report, "plants_snapped"), 3);
    assert_eq!(count(&report, "plants_estimated"), 1);
    let coal: f64 = elements
        .generators
        .iter()
        .filter(|g| g.fuel == tellegen_synth::Fuel::Coal && !g.makeup)
        .map(|g| g.pmax_mw)
        .sum();
    assert!((coal - 1200.0).abs() < 1e-9, "{coal}");
    assert!(count(&report, "makeup_units") > 0);
    assert!(elements.generators.iter().any(|g| g.makeup));
    let cap: f64 = net.generators().iter().map(|g| g.pmax).sum();
    assert!(cap >= 1.18 * 1500.0 - 1e-6);
    assert!(report
        .warnings
        .iter()
        .any(|w| w.code == "osm_makeup_generation"));

    // The planning stage keeps the mapped circuits and only measures.
    let plan = report.planning.as_ref().unwrap();
    assert_eq!(
        plan.capacity_circuits + plan.stiffening_circuits + plan.filler_circuits,
        0
    );
    assert_eq!(plan.promoted_buses, 0);

    let dc = report.dc_opf.as_ref().unwrap();
    assert_eq!(dc.status, SolveOutcome::Optimal, "{:?}", dc.message);
    assert_eq!(dc.hvdc_as_fixed_transfer, 1);
    let ac = report.ac_pf.as_ref().unwrap();
    assert_eq!(ac.status, AcPfOutcome::Converged, "{:?}", ac.message);
    assert!(
        ac.vm_min.unwrap() >= 0.9 && ac.vm_max.unwrap() <= 1.1,
        "{ac:?}"
    );
    assert_eq!(
        report
            .provenance
            .osm
            .as_ref()
            .unwrap()
            .timestamp_osm_base
            .as_deref(),
        Some("2026-10-01T00:00:00Z")
    );
}

#[test]
fn island_policies() {
    // Cut: the converters have no plant, so their islands go, with the link.
    let (module, report) = generate(
        request(OsmIslands::Cut),
        common::places(),
        Some(FIXTURE.to_owned()),
    )
    .unwrap();
    assert_eq!(module.value().buses().len(), 7);
    assert_eq!(count(&report, "buses_cut"), 2);
    assert!(module.value().hvdc().is_empty());
    assert!(report.warnings.iter().any(|w| w.code == "osm_islands_cut"));
    assert_eq!(
        report.dc_opf.as_ref().unwrap().status,
        SolveOutcome::Optimal
    );

    // Keep: the converter islands stay. They carry no load, and the HVDC link
    // between them becomes a zero-MW fixed transfer, so neither is infeasible.
    let (module, report) = generate(
        request(OsmIslands::Keep),
        common::places(),
        Some(FIXTURE.to_owned()),
    )
    .unwrap();
    assert_eq!(module.value().buses().len(), 9);
    assert_eq!(count(&report, "interconnects"), 0);
    let dc = report.dc_opf.as_ref().unwrap();
    assert_eq!(dc.islands.len(), 3);
    assert!(dc
        .islands
        .iter()
        .all(|i| i.status != "infeasible" && i.status != "no_generation"));
    assert_eq!(dc.status, SolveOutcome::Optimal, "{:?}", dc.message);
}

#[test]
fn the_bus_cap_drops_the_lowest_levels_first() {
    // Nine buses at 110 kV and above. A cap of eight drops the 110 kV level:
    // sites A, B and C keep their 380 kV buses, and the two converters stay.
    let mut req = request(OsmIslands::Connect);
    req.limits.max_bus_count = 8;
    let (module, report) = generate(req, common::places(), Some(FIXTURE.to_owned())).unwrap();
    let net = module.value();
    assert_eq!(
        net.buses().len(),
        5,
        "{:?}",
        net.buses().iter().map(|b| &b.name).collect::<Vec<_>>()
    );
    assert!(net.buses().iter().all(|b| b.base_kv == 380.0
        || b.name
            .as_deref()
            .is_some_and(|n| n.starts_with("OSM Converter"))));
    assert_eq!(count(&report, "substation_sites"), 3);
    assert_eq!(count(&report, "converters"), 2);
    assert_eq!(count(&report, "transformers"), 0);
    let capped = report
        .warnings
        .iter()
        .find(|w| w.code == "osm_bus_count_capped")
        .expect("the cap is reported");
    assert!(
        capped.message.contains("9 OpenStreetMap buses"),
        "{}",
        capped.message
    );
    assert!(
        capped.message.contains("below 380 kV"),
        "{}",
        capped.message
    );

    // At the cap nothing is dropped.
    let mut req = request(OsmIslands::Connect);
    req.limits.max_bus_count = 9;
    let (module, report) = generate(req, common::places(), Some(FIXTURE.to_owned())).unwrap();
    assert_eq!(module.value().buses().len(), 9);
    assert!(!report
        .warnings
        .iter()
        .any(|w| w.code == "osm_bus_count_capped"));
}

#[test]
fn a_highest_level_over_the_bus_cap_is_refused() {
    // The 380 kV level and the converters alone make five buses.
    let mut req = request(OsmIslands::Connect);
    req.limits.max_bus_count = 4;
    let err = generate(req, common::places(), Some(FIXTURE.to_owned()))
        .err()
        .unwrap();
    assert!(matches!(err, SynthError::InvalidRequest(_)), "{err}");
    assert!(
        err.to_string().contains("limits.max_bus_count = 4"),
        "{err}"
    );
}

#[test]
fn too_little_data_is_a_typed_refusal() {
    let thin = r#"{"elements":[
        {"type":"node","id":1,"lat":50.0,"lon":8.2,"tags":{"power":"substation","voltage":"110000"}},
        {"type":"node","id":2,"lat":50.1,"lon":8.4,"tags":{"power":"substation","voltage":"110000"}}
    ]}"#;
    let err = generate(
        request(OsmIslands::Connect),
        common::places(),
        Some(thin.to_owned()),
    )
    .err()
    .unwrap();
    assert!(
        matches!(err, SynthError::OsmInsufficient { buses: 2, .. }),
        "{err}"
    );
    assert_eq!(err.code(), "osm_insufficient");

    let no_lines = r#"{"elements":[
        {"type":"node","id":1,"lat":50.0,"lon":8.2,"tags":{"power":"substation","voltage":"110000"}},
        {"type":"node","id":2,"lat":50.1,"lon":8.4,"tags":{"power":"substation","voltage":"110000"}},
        {"type":"node","id":3,"lat":50.2,"lon":8.6,"tags":{"power":"substation","voltage":"110000"}}
    ]}"#;
    let err = generate(
        request(OsmIslands::Keep),
        common::places(),
        Some(no_lines.to_owned()),
    )
    .err()
    .unwrap();
    assert!(
        matches!(
            err,
            SynthError::OsmInsufficient {
                buses: 3,
                branches: 0,
                ..
            }
        ),
        "{err}"
    );

    let mut req = request(OsmIslands::Connect);
    req.source = Source::Osm;
    assert!(tellegen_synth::Generator::new(req, common::places(), None).is_err());
}

#[test]
fn antimeridian_halves_merge_in_one_frame() {
    // A region across 180° fetched as two documents; the line crossing the
    // seam is split at it in OSM and stitches back in the continuous frame.
    let east = r#"{"elements":[
        {"type":"node","id":1,"lat":-17.0,"lon":178.0,"tags":{"power":"substation","voltage":"132000"}},
        {"type":"node","id":2,"lat":-17.5,"lon":179.0,"tags":{"power":"substation","voltage":"132000"}},
        {"type":"way","id":10,"geometry":[{"lat":-17.0,"lon":178.0},{"lat":-17.5,"lon":179.0}],"tags":{"power":"line","voltage":"132000"}},
        {"type":"way","id":11,"geometry":[{"lat":-17.5,"lon":179.0},{"lat":-17.6,"lon":180.0}],"tags":{"power":"line","voltage":"132000"}},
        {"type":"node","id":30,"lat":-17.1,"lon":178.05,"tags":{"power":"plant","plant:source":"hydro","plant:output:electricity":"400 MW"}}
    ]}"#;
    let west = r#"{"elements":[
        {"type":"way","id":12,"geometry":[{"lat":-17.6,"lon":-180.0},{"lat":-17.8,"lon":-179.0}],"tags":{"power":"line","voltage":"132000"}},
        {"type":"node","id":3,"lat":-17.8,"lon":-179.0,"tags":{"power":"substation","voltage":"132000"}}
    ]}"#;
    let mut req = SynthRequest::new(Region::bbox(177.0, -19.0, 182.0, -16.0, "Seam"));
    req.source = Source::Osm;
    // Small enough for one mapped 132 kV line over about 120 km (St. Clair
    // limits it near 110 MW); OSM mode keeps the mapped circuits.
    req.demand = DemandSpec::TotalMw { mw: 60.0 };
    let docs = format!("[{east},{west}]");
    let (module, report) = generate(req, common::places(), Some(docs)).unwrap();
    let net = module.value();
    assert_eq!(net.buses().len(), 3);
    assert!(net
        .buses()
        .iter()
        .all(|b| (-180.0..=180.0).contains(&b.location.unwrap().x)));
    assert_eq!(
        count(&report, "lines_stitched"),
        1,
        "the two halves of the seam line"
    );
    assert_eq!(
        count(&report, "interconnects"),
        0,
        "the stitched line joins both sides"
    );
    assert_eq!(
        report.dc_opf.as_ref().unwrap().status,
        SolveOutcome::Optimal
    );
}

#[test]
fn absurd_circuit_counts_are_clamped() {
    // Ways 301 and 302 claim four billion circuits, the cable a quintillion
    // conductors. Every circuit becomes a branch row, so unclamped this never
    // finishes.
    let doc = FIXTURE
        .replacen("\"circuits\": \"1\"", "\"circuits\": \"4294967295\"", 2)
        .replace("\"cables\": \"6\"", "\"cables\": \"1e18\"");
    let (module, _) = generate(request(OsmIslands::Connect), common::places(), Some(doc)).unwrap();
    assert!(module.value().branches().len() < 100);
}
