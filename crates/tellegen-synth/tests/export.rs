//! Export of generated grids: the GridFM graphkit document, and the PowerIO
//! writers through a tellegen Study (MATPOWER round trip to the same DC
//! objective; PSS/E v35, PowerModels and pandapower JSON emit, the last with
//! bus coordinates).

mod common;

use powerio::Source;
use powerio_prob::DcOpfInstance;
use tellegen::{Problem, SolveRequest, Study};
use tellegen_synth::{generate, Generator};

fn grid(
    buses: u32,
) -> (
    powerio::PioModule<powerio::BalancedNetwork>,
    tellegen_synth::GenerationReport,
) {
    let req = common::request(common::gulf_of_guinea_box(), 5, Some(buses));
    generate(req, common::places(), None).expect("generate")
}

#[test]
fn gridfm_document_matches_the_network() {
    let req = common::request(common::gulf_of_guinea_box(), 5, Some(30));
    let mut session = Generator::new(req, common::places(), None).unwrap();
    assert!(
        session.export_gridfm().is_err(),
        "no network before electrify"
    );
    while !session.is_done() {
        session.step().unwrap();
    }
    let doc: serde_json::Value = serde_json::from_str(&session.export_gridfm().unwrap()).unwrap();
    let net = session.network().unwrap();
    assert_eq!(doc["format"], "gridfm-graphkit");
    assert_eq!(doc["n_bus"], net.buses().len());
    assert_eq!(doc["bus"].as_array().unwrap().len(), net.buses().len());
    assert_eq!(
        doc["branch"].as_array().unwrap().len(),
        net.branches().len()
    );
    assert_eq!(
        doc["edge_index"][0].as_array().unwrap().len(),
        2 * net.branches().len()
    );
    assert_eq!(doc["gen"].as_array().unwrap().len(), net.generators().len());
    assert!(doc["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c.is_array()));
    // One reference bus per island, and the DC OPF flows fill the P column.
    let refs = doc["bus"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[7] == 1.0)
        .count();
    assert_eq!(refs, 1);
    let flow: f64 = doc["branch"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].as_f64().unwrap().abs())
        .sum();
    assert!(flow > 0.0);
    // Shunt banks appear as bus susceptance.
    let bs: f64 = doc["bus"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[13].as_f64().unwrap().abs())
        .sum();
    assert_eq!(bs > 0.0, !net.shunts().is_empty());
}

#[test]
fn matpower_export_reparses_and_solves_to_the_same_objective() {
    let (module, report) = grid(40);
    let reported = report.dc_opf.as_ref().unwrap().objective.unwrap();
    let study = Study::from_module(module, Problem::DcOpf).unwrap();
    let solved = study.solution().objective.unwrap();
    assert!((solved - reported).abs() <= 1e-6 * reported.abs().max(1.0));

    let exported = study.export("matpower").unwrap();
    let source = Source::from_memory("<case>", exported.text.into_bytes())
        .unwrap()
        .with_format(powerio::FormatId::new("matpower").unwrap());
    let parsed = tellegen::ir::balanced_module(powerio::parse(source).unwrap()).unwrap();
    let instance = DcOpfInstance::from_network(parsed.value().clone()).unwrap();
    let response = tellegen::solve_instance(
        &instance,
        &SolveRequest {
            formulation: Problem::DcOpf,
            ..Default::default()
        },
    )
    .unwrap();
    let reparsed = response.objective.unwrap();
    assert!(
        (reparsed - reported).abs() <= 1e-4 * reported.abs().max(1.0),
        "{reparsed} vs {reported}"
    );
}

#[test]
fn psse35_and_json_writers_emit_and_pandapower_keeps_coordinates() {
    let (module, _) = grid(30);
    let study = Study::from_module(module, Problem::DcOpf).unwrap();
    for format in ["psse35", "powermodels-json", "pandapower-json"] {
        let exported = study
            .export(format)
            .unwrap_or_else(|e| panic!("{format}: {e}"));
        assert!(!exported.text.is_empty(), "{format}");
    }
    let pandapower = study.export("pandapower-json").unwrap();
    assert!(
        pandapower.text.contains("Point"),
        "pandapower writes bus geo as GeoJSON points"
    );
}
