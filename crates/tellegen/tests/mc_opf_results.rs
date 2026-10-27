#![cfg(feature = "mc-opf")]

use num_complex::Complex64 as C;
use powerio::{McAcOpfInstance, PioModule, PioValue};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicBool, Arc},
};
use tellegen::{mc_opf::*, solve_mc_ac_opf_instance, McOpfOptions};

fn case(bundle: &str, name: &str) -> powerio::MulticonductorNetwork {
    let text = match bundle {
        "components" => include_str!("data/mc_opf/components.json"),
        "equipment" => include_str!("data/mc_opf/equipment.json"),
        "controls" => include_str!("data/mc_opf/controls.json"),
        _ => unreachable!(),
    };
    let cases: Value = serde_json::from_str(text).unwrap();
    let module = powerio::parse_with_options(
        powerio::Source::from_memory("case.bmopf.json", serde_json::to_vec(&cases[name]).unwrap())
            .unwrap(),
        &powerio::ParseOptions::default()
            .format("bmopf-json")
            .unwrap(),
    )
    .unwrap();
    validate_mc_opf_input_diagnostics(&module).unwrap();
    let PioValue::MulticonductorNetwork(net) = module.into_value() else {
        panic!("network")
    };
    net
}
fn instance(net: powerio::MulticonductorNetwork) -> Arc<McAcOpfInstance> {
    Arc::new(McAcOpfInstance::from_network(net).unwrap())
}
fn complex(v: [f64; 2]) -> C {
    C::new(v[0], v[1])
}
fn key(t: &McOpfTerminalId) -> (String, String) {
    (t.bus.clone(), t.terminal.clone())
}

/// Reconstruct public-ledger KCL without preparation indices or solver columns.
fn assert_ledgers(result: &McOpfResult) {
    let voltage: BTreeMap<_, _> = result
        .terminals
        .iter()
        .map(|t| (key(&t.terminal), complex(t.voltage_v)))
        .collect();
    let mut kcl: BTreeMap<_, C> = voltage.keys().cloned().map(|k| (k, C::default())).collect();
    let power = |t: &McOpfTerminalId, i: [f64; 2], p: [f64; 2]| {
        assert!((voltage[&key(t)] * complex(i).conj() - complex(p)).norm() < 1e-7);
    };
    for b in &result.branches {
        for (ts, is, ps) in [
            (&b.from_terminals, &b.current_from_a, &b.power_from_va),
            (&b.to_terminals, &b.current_to_a, &b.power_to_va),
        ] {
            for ((t, &i), &p) in ts.iter().zip(is).zip(ps) {
                power(t, i, p);
                *kcl.get_mut(&key(t)).unwrap() -= complex(i);
            }
        }
    }
    for d in &result.devices {
        let sign = if d.kind == powerio_matrix::McOpfDeviceKind::Load {
            -1.0
        } else {
            1.0
        };
        let mut from_coils: BTreeMap<_, C> =
            d.terminals.iter().map(|t| (key(t), C::default())).collect();
        for ((axis, &i), &p) in d.coils.iter().zip(&d.coil_current_a).zip(&d.coil_power_va) {
            let u = voltage[&key(&axis.positive)]
                - axis
                    .negative
                    .as_ref()
                    .map_or(C::default(), |t| voltage[&key(t)]);
            assert!((u * complex(i).conj() - complex(p)).norm() < 1e-7);
            *from_coils.get_mut(&key(&axis.positive)).unwrap() += complex(i);
            if let Some(n) = &axis.negative {
                *from_coils.get_mut(&key(n)).unwrap() -= complex(i);
            }
        }
        for ((t, &i), &p) in d
            .terminals
            .iter()
            .zip(&d.terminal_current_a)
            .zip(&d.terminal_power_va)
        {
            power(t, i, p);
            assert!((from_coils[&key(t)] - complex(i)).norm() < 1e-8);
            *kcl.get_mut(&key(t)).unwrap() += sign * complex(i);
        }
    }
    for tx in &result.transformers {
        for ((axis, &i), &p) in tx
            .coils
            .iter()
            .zip(&tx.coil_current_a)
            .zip(&tx.coil_power_va)
        {
            let u = voltage[&key(&axis.positive)]
                - axis
                    .negative
                    .as_ref()
                    .map_or(C::default(), |t| voltage[&key(t)]);
            assert!((u * complex(i).conj() - complex(p)).norm() < 1e-7);
            *kcl.get_mut(&key(&axis.positive)).unwrap() -= complex(i);
            if let Some(n) = &axis.negative {
                *kcl.get_mut(&key(n)).unwrap() += complex(i);
            }
        }
        for port in &tx.ports {
            let u = voltage[&key(&port.positive)]
                - port
                    .negative
                    .as_ref()
                    .map_or(C::default(), |t| voltage[&key(t)]);
            assert!((u * complex(port.current_a).conj() - complex(port.power_va)).norm() < 1e-7);
        }
    }
    for sh in &result.shunts {
        for ((t, &i), &p) in sh.terminals.iter().zip(&sh.current_a).zip(&sh.power_va) {
            power(t, i, p);
            *kcl.get_mut(&key(t)).unwrap() -= complex(i);
        }
    }
    for bus in result.solution.instance().network().buses() {
        for terminal in &bus.terminals {
            if !bus.grounded.contains(terminal) {
                assert!(
                    kcl[&(bus.id.clone(), terminal.clone())].norm() < 1e-5,
                    "{}:{}",
                    bus.id,
                    terminal
                );
            }
        }
    }
}

#[test]
fn complete_results_roundtrip_with_coil_terminal_and_rating_port_axes() {
    for (bundle, name) in [
        ("components", "ibr_four_leg"),
        ("components", "capacitor_delta"),
        ("components", "transformer_wye_delta"),
        ("equipment", "center_neutral_impedance"),
        ("equipment", "n_winding_mixed_connections"),
        ("equipment", "delta_wye_neutral_rating"),
        ("controls", "tap_regulator_b"),
    ] {
        let mut net = case(bundle, name);
        net.buses_mut().reverse();
        let options = McOpfOptions {
            objective_scale: if bundle == "controls" { 1000.0 } else { 1.0 },
            collect_profile: true,
            ..Default::default()
        };
        let result = solve_mc_ac_opf_instance(instance(net), &options).unwrap();
        assert_ledgers(&result);
        let text = result.to_module_json().unwrap();
        let back = McOpfResult::from_module_json(&text).unwrap();
        assert_ledgers(&back);
        let before = tellegen::ir::deserialize_module(&text).unwrap();
        let after = tellegen::ir::deserialize_module(&back.to_module_json().unwrap()).unwrap();
        assert_eq!(before.extensions(), after.extensions(), "{name}");
        assert_eq!(result.solution.objective(), back.solution.objective());
        assert_eq!(
            result.solution.instance().network().transformers().len(),
            back.solution.instance().network().transformers().len()
        );
        if bundle == "controls" {
            assert!(back.transformers.iter().any(|t| t.tap.is_some()));
        }
        if name == "center_neutral_impedance" {
            assert!(!back.shunts.is_empty());
        }
        if name == "delta_wye_neutral_rating" {
            assert!(back
                .transformers
                .iter()
                .flat_map(|t| &t.ports)
                .any(|p| p.name.ends_with(":neutral")));
        }
    }
}

#[test]
fn module_solve_preserves_input_findings_and_unrelated_extensions() {
    let mut module = PioModule::new(PioValue::McAcOpfInstance(
        (*instance(case("components", "capacitor_wye"))).clone(),
    ));
    let note = powerio::Diagnostic::new(
        powerio::DiagnosticCode::new("READ.TEST.LOWERED").unwrap(),
        powerio::DiagnosticSeverity::Remark,
        "lossless capacitor lowering",
    )
    .with_target("/network/shunts/0")
    .unwrap();
    module.add_diagnostic(note).unwrap();
    module
        .insert_extension("org.example.provenance", json!({"source":"regression"}))
        .unwrap();
    let input = tellegen::ir::serialize_module(&module).unwrap();
    let output = solve_mc_ac_opf_module_json(&input, &Default::default()).unwrap();
    let parsed = tellegen::ir::deserialize_module(&output).unwrap();
    assert_eq!(
        parsed.extensions()["org.example.provenance"],
        json!({"source":"regression"})
    );
    assert_eq!(parsed.diagnostics()[0].code(), "READ.TEST.LOWERED");
    // Targets into the replaced value are removed in PowerIO records; the
    // result report retains the original input finding for diagnosis.
    assert!(parsed.diagnostics()[0].target().is_none());
    let result = McOpfResult::from_module_json(&output).unwrap();
    assert_eq!(
        result.diagnostics[0].target.as_deref(),
        Some("/network/shunts/0")
    );
    assert!(!result.shunts.is_empty());
    let saved_again = tellegen::ir::deserialize_module(&result.to_module_json().unwrap()).unwrap();
    assert_eq!(parsed.extensions(), saved_again.extensions());
    assert_eq!(saved_again.diagnostics()[0].code(), "READ.TEST.LOWERED");
}

#[test]
fn malformed_incomplete_or_mismatched_saved_reports_are_rejected() {
    let r = solve_mc_ac_opf_instance(
        instance(case("components", "ibr_four_leg")),
        &Default::default(),
    )
    .unwrap();
    let saved = r.to_module_json().unwrap();
    for change in 0..7 {
        let mut module = tellegen::ir::deserialize_module(&saved).unwrap();
        let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
        match change {
            0 => report["version"] = json!(99),
            1 => {
                report.as_object_mut().unwrap().remove("transformers");
            }
            2 => report["terminals"][0]["terminal"]["terminal"] = json!("not-a-terminal"),
            3 => report["devices"][0]["terminal_current_a"] = json!([]),
            4 => report["options"]["power_base_va"] = json!(2000),
            5 => {
                let source = report["devices"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["kind"] == "Source")
                    .unwrap();
                source["terminal_power_va"][0][0] = json!(123456.0);
            }
            6 => report["residuals"]["kcl_pu"] = json!(10.0),
            _ => unreachable!(),
        }
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report)
            .unwrap();
        let error =
            McOpfResult::from_module_json(&tellegen::ir::serialize_module(&module).unwrap())
                .unwrap_err();
        assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
        assert_eq!(error.stage, McOpfStage::Import);
    }
    let legacy = tellegen::ir::serialize_module(&PioModule::new(PioValue::McAcOpfSolution(
        r.solution.clone(),
    )))
    .unwrap();
    assert!(McOpfResult::from_module_json(&legacy).is_err());
    let mut bad = r;
    bad.devices[0].terminal_current_a[0][0] = f64::INFINITY;
    assert_eq!(
        bad.to_module_json().unwrap_err().kind,
        McOpfErrorKind::SerializationFailure
    );
}

#[test]
fn failure_categories_are_structured_and_never_contain_a_solution() {
    let net = case("components", "ibr_four_leg");
    let cancelled = solve_mc_ac_opf_instance_cancellable(
        instance(net.clone()),
        &Default::default(),
        Some(Arc::new(AtomicBool::new(true))),
    )
    .unwrap_err();
    assert_eq!(cancelled.kind, McOpfErrorKind::Cancelled);
    let bad_options = solve_mc_ac_opf_instance(
        instance(net.clone()),
        &McOpfOptions {
            voltage_base_v: f64::NAN,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(bad_options.kind, McOpfErrorKind::InvalidInput);
    assert_eq!(
        bad_options.diagnostics[0].target.as_deref(),
        Some("/options/voltage_base_v")
    );
    let stopped = solve_mc_ac_opf_instance(
        instance(net.clone()),
        &McOpfOptions {
            max_iterations: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(stopped.kind, McOpfErrorKind::NumericalFailure);
    assert!(stopped.solver_status.is_some());
    let mut unsupported = net;
    unsupported.loads_mut()[0]
        .extras
        .insert("unknown_physics".into(), json!(1));
    let unsupported =
        solve_mc_ac_opf_instance(instance(unsupported), &Default::default()).unwrap_err();
    assert_eq!(unsupported.kind, McOpfErrorKind::UnsupportedInput);
    assert_eq!(unsupported.stage, McOpfStage::Preparation);
    assert!(unsupported
        .diagnostics
        .iter()
        .any(|d| d.code.contains("UNSUPPORTED")));
    for error in [cancelled, bad_options, stopped, unsupported] {
        let value = serde_json::to_value(&error).unwrap();
        assert!(value.get("solution").is_none());
        let back: McOpfError = serde_json::from_value(value).unwrap();
        assert_eq!(back.kind, error.kind);
        assert_eq!(back.stage, error.stage);
    }
}

#[test]
fn lossy_input_errors_preserve_codes_targets_and_all_findings() {
    let mut module = PioModule::new(PioValue::McAcOpfInstance(
        (*instance(case("components", "ibr_single_phase"))).clone(),
    ));
    for (code, target) in [
        ("READ.TEST.FIELD_DROPPED", "/network/ibrs/0"),
        ("READ.TEST.MALFORMED", "/network/buses/0"),
    ] {
        module
            .add_diagnostic(
                powerio::Diagnostic::new(
                    powerio::DiagnosticCode::new(code).unwrap(),
                    powerio::DiagnosticSeverity::Warning,
                    "physical data rejected",
                )
                .with_target(target)
                .unwrap(),
            )
            .unwrap();
    }
    let input = tellegen::ir::serialize_module(&module).unwrap();
    let error = solve_mc_ac_opf_module_json(&input, &Default::default()).unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
    assert_eq!(error.diagnostics.len(), 2);
    assert_eq!(error.diagnostics[0].code, "READ.TEST.FIELD_DROPPED");
    assert_eq!(
        error.diagnostics[0].target.as_deref(),
        Some("/network/ibrs/0")
    );
    let parse = solve_mc_ac_opf_module_json("{", &Default::default()).unwrap_err();
    assert_eq!(parse.stage, McOpfStage::Input);
    assert!(!parse.diagnostics.is_empty());
}

#[test]
fn a_later_solver_failure_keeps_lossless_input_findings() {
    let mut module = PioModule::new(PioValue::McAcOpfInstance(
        (*instance(case("components", "ibr_four_leg"))).clone(),
    ));
    module
        .add_diagnostic(powerio::Diagnostic::new(
            powerio::DiagnosticCode::new("READ.TEST.NORMALIZED").unwrap(),
            powerio::DiagnosticSeverity::Remark,
            "lossless normalization",
        ))
        .unwrap();
    let input = tellegen::ir::serialize_module(&module).unwrap();
    let error = solve_mc_ac_opf_module_json(
        &input,
        &McOpfOptions {
            max_iterations: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::NumericalFailure);
    assert_eq!(error.diagnostics[0].code, "READ.TEST.NORMALIZED");
}
