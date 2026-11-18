//! Synthetic raw OpenDSS snapshots, independently solved by the pinned backend.
#![cfg(feature = "mc-pf")]
use serde::Deserialize;
use std::collections::BTreeMap;
use tellegen::{McPfOptions, McPfResult};

#[derive(Deserialize)]
struct Reference {
    name: String,
    source: String,
    #[serde(default)]
    fixed_controls: bool,
    #[serde(default)]
    floating_bus: Option<String>,
    #[serde(default)]
    centered_on_ground: bool,
    voltages: BTreeMap<String, [f64; 2]>,
    currents: BTreeMap<String, Current>,
    transformers: BTreeMap<String, Transformer>,
}
#[derive(Deserialize)]
struct Transformer {
    ports: BTreeMap<String, [f64; 2]>,
    losses: [f64; 2],
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
fn generator_snapshot_is_view_only_before_solver_preparation() {
    let input = module("new circuit.qc basekv=.4\nnew generator.g phases=3 bus1=sourcebus kv=.4 kw=3 kvar=1 model=1\n");
    let reason = tellegen::validate_mc_module(&input).unwrap_err();
    assert!(reason.contains("generator or IBR injections"), "{reason}");
    let saved = tellegen::ir::serialize_module(&input).unwrap();
    assert!(tellegen::validate_mc_module_json(&saved).is_err());
    assert!(tellegen::solve_mc_module_json(&saved, &Default::default()).is_err());
}

#[test]
fn capability_rejects_missing_voltage_dependent_load_base() {
    for model in [
        powerio_dist::DistLoadVoltageModel::ConstantCurrent { v_nom: vec![] },
        powerio_dist::DistLoadVoltageModel::ConstantImpedance { v_nom: vec![] },
        powerio_dist::DistLoadVoltageModel::Exponential {
            v_nom: vec![],
            gamma_p: vec![0.7],
            gamma_q: vec![2.3],
        },
    ] {
        let mut input = module(
            "new circuit.qc basekv=.4\nnew load.ld phases=3 bus1=sourcebus kv=.4 kw=3 kvar=1\n",
        );
        let powerio::PioValue::MulticonductorNetwork(network) = input.value_mut() else {
            panic!()
        };
        network.loads_mut()[0].voltage_model = model;
        let reason = tellegen::validate_mc_module(&input).unwrap_err();
        assert!(reason.contains("nominal branch voltage"), "{reason}");
        let saved = tellegen::ir::serialize_module(&input).unwrap();
        assert!(tellegen::validate_mc_module_json(&saved).is_err());
        assert!(tellegen::solve_mc_module_json(&saved, &Default::default()).is_err());
    }
}

#[test]
fn finite_sources_bounded_loads_four_wires_and_transformer_match_opendss() {
    let report: Report = serde_json::from_str(include_str!("data/dss_snapshots.json")).unwrap();
    assert_eq!(report.cases.len(), 53);
    for case in report.cases {
        let input = module(&case.source);
        let input = if case.fixed_controls {
            assert!(tellegen::validate_mc_module(&input).is_err());
            tellegen::prepare_mc_fixed_controls(&input).unwrap()
        } else {
            input
        };
        tellegen::validate_mc_module(&input).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let text = tellegen::ir::serialize_module(&input).unwrap();
        for envelope in [false, true] {
            let options = McPfOptions {
                // Absolute volts: the delta winding has a weak common-mode reference.
                // Independent voltage/current comparisons below stay at 20 micro-units.
                tolerance: if case.name.starts_with("three_") {
                    1e-6
                } else {
                    1e-10
                },
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
            let solved = |bus: &str, terminal: &str| {
                result
                    .terminals
                    .iter()
                    .find(|t| t.bus.eq_ignore_ascii_case(bus) && t.terminal == terminal)
                    .unwrap_or_else(|| panic!("{}: missing {bus}.{terminal}", case.name))
                    .voltage
            };
            // A bus with no path to ground is recorded between terminals,
            // relative to terminal 1. With a nonzero ppm_antifloat its voltages
            // also centre on ground; with zero the voltage to ground is not
            // defined and each solver returns a different one.
            if let Some(bus) = &case.floating_bus {
                let on_bus: Vec<_> = result
                    .terminals
                    .iter()
                    .filter(|t| t.bus.eq_ignore_ascii_case(bus))
                    .collect();
                let re: f64 = on_bus.iter().map(|t| t.voltage.re).sum();
                let im: f64 = on_bus.iter().map(|t| t.voltage.im).sum();
                let centered = re.hypot(im) / (on_bus.len() as f64) < 1e-3;
                if case.centered_on_ground {
                    assert!(centered, "{}: {bus} is not centred on ground", case.name);
                }
            }
            for (node, expected) in &case.voltages {
                let (bus, terminal) = node.rsplit_once('.').unwrap();
                let mut actual = solved(bus, terminal);
                if case.floating_bus.as_deref() == Some(bus) {
                    let reference = solved(bus, "1");
                    actual.re -= reference.re;
                    actual.im -= reference.im;
                }
                assert!(
                    (actual.re - expected[0]).hypot(actual.im - expected[1]) < 2e-5,
                    "{}: {node}: {actual:?} != {expected:?}",
                    case.name
                );
            }
            for (name, expected) in &case.transformers {
                let ports: Vec<_> = result
                    .element_ports
                    .iter()
                    .filter(|port| {
                        port.kind == "transformer" && port.element.eq_ignore_ascii_case(name)
                    })
                    .collect();
                for (label, current) in &expected.ports {
                    let (bus, terminal) = label.rsplit_once('.').unwrap();
                    let port = ports
                        .iter()
                        .find(|port| {
                            port.bus.eq_ignore_ascii_case(bus) && port.terminal == terminal
                        })
                        .unwrap();
                    assert!(
                        (port.current_into_element.re - current[0])
                            .hypot(port.current_into_element.im - current[1])
                            < 2e-5,
                        "{}: transformer {name} port {label}",
                        case.name
                    );
                }
                let p: f64 = ports.iter().map(|port| port.power_into_element.re).sum();
                let q: f64 = ports.iter().map(|port| port.power_into_element.im).sum();
                assert!(
                    (p - expected.losses[0]).hypot(q - expected.losses[1]) < 0.01,
                    "{}: transformer {name} losses {p},{q} != {:?}",
                    case.name,
                    expected.losses
                );
            }
            for (load, expected) in &case.currents {
                // A delta load has two coils on each terminal; OpenDSS reports
                // the terminal's total current.
                let ports: Vec<_> = result
                    .element_ports
                    .iter()
                    .filter(|p| {
                        p.element.eq_ignore_ascii_case(load)
                            && p.kind == "load"
                            && p.terminal == expected.terminal
                    })
                    .collect();
                assert!(!ports.is_empty(), "{}: {load}: no port", case.name);
                let actual = ports.iter().fold((0.0, 0.0), |sum, p| {
                    (
                        sum.0 + p.current_into_element.re,
                        sum.1 + p.current_into_element.im,
                    )
                });
                assert!(
                    (actual.0 - expected.value[0]).hypot(actual.1 - expected.value[1]) < 2e-5,
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
fn disabled_line_is_kept_as_a_record_and_left_out_of_the_circuit() {
    let base = "new circuit.qc basekv=.4\nnew line.a phases=3 bus1=sourcebus bus2=lb r1=.1 x1=.2 r0=.2 x0=.4 c1=0 c0=0 length=1 units=km\nnew load.ld phases=3 bus1=lb kv=.4 kw=3 kvar=1 model=1\n";
    let tie = "new line.tie phases=3 bus1=sourcebus bus2=lb r1=.1 x1=.2 r0=.2 x0=.4 c1=0 c0=0 length=1 units=km";
    let voltages = |text: &str| {
        let input = module(text);
        tellegen::validate_mc_module(&input).unwrap();
        let result: McPfResult = serde_json::from_str(
            &tellegen::solve_mc_module_json(
                &tellegen::ir::serialize_module(&input).unwrap(),
                &Default::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(result.converged);
        serde_json::to_value(&result.terminals).unwrap()
    };
    let without = voltages(base);
    for disabled in [
        format!("{base}{tie} enabled=n\n"),
        format!("{base}{tie}\ndisable line.tie\n"),
    ] {
        assert_eq!(voltages(&disabled), without, "{disabled}");
    }
    assert_ne!(voltages(&format!("{base}{tie}\n")), without);

    // The retained record cannot be switched back on without its branch.
    let mut input = module(&format!("{base}{tie} enabled=n\n"));
    let powerio::PioValue::MulticonductorNetwork(net) = input.value_mut() else {
        panic!()
    };
    assert_eq!(net.untyped_objects().len(), 1);
    net.untyped_objects_mut()[0]
        .props
        .push((Some("enabled".into()), "yes".into()));
    assert!(tellegen::validate_mc_module(&input).is_err());
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
fn unsupported_transformer_properties_stay_view_only_and_leadlag_reads() {
    let unit = |extra: &str| {
        module(&format!("new circuit.qc basekv=11\nnew transformer.t phases=3 windings=2 buses=[sourcebus lb] conns=[delta wye] kvs=[11 .4] kvas=[100 100] xhl=4 {extra}\n"))
    };
    for extra in [
        "windings=4 buses=[sourcebus lb lc ld] kvs=[11 .4 .4 .4] kvas=[100 100 100 100]",
        "%imag=garbage",
        "ppm_antifloat=garbage",
    ] {
        assert!(
            tellegen::validate_mc_module(&unit(extra)).is_err(),
            "{extra}"
        );
    }
    // PowerIO's delta orientation reads LeadLag: Lead reverses the rotation.
    tellegen::validate_mc_module(&unit("leadlag=lead")).unwrap();
}
