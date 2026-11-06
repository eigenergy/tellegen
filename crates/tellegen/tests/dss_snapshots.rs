//! Synthetic raw OpenDSS snapshots, independently solved by the pinned backend.
#![cfg(feature = "mc-pf")]
use serde::Deserialize;
use std::collections::BTreeMap;
use tellegen::{McPfOptions, McPfResult};

#[derive(Deserialize)]
struct Reference {
    name: String,
    source: String,
    voltages: BTreeMap<String, [f64; 2]>,
    currents: BTreeMap<String, Current>,
}
#[derive(Deserialize)]
struct Current {
    terminal: String,
    value: [f64; 2],
}
#[derive(Deserialize)]
struct Report {
    cases: Vec<Reference>,
}

fn module(text: &str) -> powerio::PioModule<powerio::PioValue> {
    powerio::parse(powerio::Source::from_memory("snapshot.dss", text.as_bytes().to_vec()).unwrap())
        .unwrap()
}

#[test]
fn finite_sources_bounded_loads_four_wires_and_transformer_match_opendss() {
    let report: Report = serde_json::from_str(include_str!("data/dss_snapshots.json")).unwrap();
    assert_eq!(report.cases.len(), 38);
    for case in report.cases {
        let input = module(&case.source);
        tellegen::validate_mc_module(&input).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let text = tellegen::ir::serialize_module(&input).unwrap();
        for envelope in [false, true] {
            let options = McPfOptions {
                tolerance: 1e-10,
                absolute_kcl_tolerance: 1e-7,
                relative_kcl_tolerance: 1e-9,
                voltage_envelope: envelope,
                ..Default::default()
            };
            let result: McPfResult = serde_json::from_str(
                &tellegen::solve_mc_module_json(&text, &options)
                    .unwrap_or_else(|e| panic!("{}: {e}", case.name)),
            )
            .unwrap();
            assert!(
                result.converged,
                "{}: residual {}",
                case.name, result.physical_kcl_residual
            );
            assert!(result.physical_kcl_residual < 1e-6, "{}", case.name);
            for (node, expected) in &case.voltages {
                let (bus, terminal) = node.rsplit_once('.').unwrap();
                let actual = result
                    .terminals
                    .iter()
                    .find(|t| t.bus.eq_ignore_ascii_case(bus) && t.terminal == terminal)
                    .unwrap_or_else(|| panic!("{}: missing {node}", case.name))
                    .voltage;
                assert!(
                    (actual.re - expected[0]).hypot(actual.im - expected[1]) < 2e-5,
                    "{}: {node}: {actual:?} != {expected:?}",
                    case.name
                );
            }
            for (load, expected) in &case.currents {
                let actual = result
                    .element_ports
                    .iter()
                    .find(|p| {
                        p.element.eq_ignore_ascii_case(load)
                            && p.kind == "load"
                            && p.terminal == expected.terminal
                    })
                    .unwrap()
                    .current_into_element;
                assert!(
                    (actual.re - expected.value[0]).hypot(actual.im - expected.value[1]) < 2e-5,
                    "{}: {load}: {actual:?} != {:?}",
                    case.name,
                    expected.value
                );
            }
        }
    }
}

#[test]
fn passive_records_stay_retained_but_controls_and_incomplete_sources_are_blocked() {
    let base =
        "new circuit.qc basekv=.4\nnew load.ld phases=3 bus1=sourcebus kv=.4 kw=3 kvar=1 model=1\n";
    let passive = module(&format!(
        "{base}new loadshape.profile npts=2 mult=[1 2]\nnew monitor.m element=load.ld terminal=1\n"
    ));
    tellegen::validate_mc_module(&passive).unwrap();
    let restored =
        tellegen::ir::deserialize_module(&tellegen::ir::serialize_module(&passive).unwrap())
            .unwrap();
    let powerio::PioValue::MulticonductorNetwork(net) = restored.value() else {
        panic!()
    };
    assert_eq!(net.untyped_objects().len(), 2);
    tellegen::validate_mc_module(&restored).unwrap();
    for extra in [
        "new regcontrol.reg transformer=missing",
        "new relay.r monitoredobj=load.ld",
        "redirect missing.dss",
    ] {
        let blocked = module(&format!("{base}{extra}\n"));
        assert!(tellegen::validate_mc_module(&blocked).is_err(), "{extra}");
        assert!(
            tellegen::solve_mc_module_json(
                &tellegen::ir::serialize_module(&blocked).unwrap(),
                &Default::default()
            )
            .is_err(),
            "{extra}"
        );
    }
}

#[test]
fn floating_source_reference_is_rejected_by_capability_and_direct_solver() {
    let mut input = module("new circuit.qc basekv=.4\n");
    let powerio::PioValue::MulticonductorNetwork(net) = input.value_mut() else {
        panic!()
    };
    let bus = net.sources()[0].bus.clone();
    net.sources_mut()[0].reference_terminal = Some("0".into());
    let source_bus = net.buses_mut().iter_mut().find(|b| b.id == bus).unwrap();
    if !source_bus.terminals.iter().any(|t| t == "0") {
        source_bus.terminals.push("0".into());
    }
    source_bus.grounded.retain(|t| t != "0");
    let instance = powerio_prob::McAcPfInstance::from_network(net.clone()).unwrap();
    assert!(tellegen::validate_mc_module(&input)
        .unwrap_err()
        .contains("floating"));
    assert!(
        tellegen::solve_mc_ac_pf_instance(&instance, &Default::default())
            .unwrap_err()
            .contains("floating")
    );
}

#[test]
fn unsupported_transformer_properties_and_three_windings_are_view_only() {
    for extra in [
        "windings=3 buses=[sourcebus lb lc] kvs=[11 .4 .4] kvas=[100 100 100]",
        "leadlag=lead",
        "%imag=garbage",
        "ppm_antifloat=50",
    ] {
        let input = module(&format!("new circuit.qc basekv=11\nnew transformer.t phases=3 windings=2 buses=[sourcebus lb] conns=[delta wye] kvs=[11 .4] kvas=[100 100] xhl=4 {extra}\n"));
        assert!(tellegen::validate_mc_module(&input).is_err(), "{extra}");
    }
}
