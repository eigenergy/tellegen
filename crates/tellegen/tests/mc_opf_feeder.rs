//! One combined feeder, five solves, one sensitivity factorization. No Julia in CI.
#![cfg(feature = "mc-opf")]
use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
use powerio_matrix::McOpfDeviceKind;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Instant};
use tellegen::*;

const INPUT: &[u8] = include_bytes!("data/mc_opf/feeder.json");
fn network() -> powerio_dist::MulticonductorNetwork {
    let module = powerio::parse_with_options(
        Source::from_memory("feeder.bmopf.json", INPUT.to_vec()).unwrap(),
        &ParseOptions::default().format("bmopf-json").unwrap(),
    )
    .unwrap();
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let PioValue::MulticonductorNetwork(n) = module.into_value() else {
        panic!("network")
    };
    // Structural cost guard: fixture expansion requires an explicit budget review.
    assert_eq!(
        (
            n.buses().len(),
            n.lines().len(),
            n.loads().len(),
            n.ibrs().len()
        ),
        (4, 2, 2, 1)
    );
    n
}
fn options() -> McOpfOptions {
    McOpfOptions {
        collect_duals: true,
        max_iterations: 100,
        ..Default::default()
    }
}
fn solve(n: powerio_dist::MulticonductorNetwork) -> McOpfResult {
    solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(n).unwrap()),
        // Perturbed and fresh comparison solves need physical results only.
        &McOpfOptions {
            collect_duals: false,
            ..options()
        },
    )
    .unwrap()
}
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b}, tolerance {tol}");
}
fn reference(r: &McOpfResult) {
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/mc_opf/feeder-reference.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let p = &expected["provenance"];
    let hash: String = Sha256::digest(INPUT)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(p["input_sha256"], hash);
    assert_eq!(
        p["bmopftools_commit"],
        "a8b52e069bfd4a7a57434c91bc0470ac03cfdd55"
    );
    assert_eq!(p["units"], "V, A, W, var, currency/hour");
    assert_eq!(expected["feasible"], true);
    let tolerance = |name: &str| p["tolerances"][name].as_f64().unwrap();
    near(
        r.solution.objective(),
        expected["objective"].as_f64().unwrap(),
        tolerance("objective"),
    );
    for v in &r.terminals {
        let e = &expected["bus"][&v.terminal.bus][&v.terminal.terminal];
        let mag = r
            .solution
            .terminal_voltage_magnitude(&v.terminal.bus, &v.terminal.terminal)
            .unwrap();
        let angle = r
            .solution
            .terminal_voltage_angle(&v.terminal.bus, &v.terminal.terminal)
            .unwrap();
        near(
            mag * angle.cos(),
            e["vr"].as_f64().unwrap(),
            tolerance("voltage_v"),
        );
        near(
            mag * angle.sin(),
            e["vi"].as_f64().unwrap(),
            tolerance("voltage_v"),
        );
    }
    let input: Value = serde_json::from_slice(INPUT).unwrap();
    let mut loss = 0.;
    let mut reference_loss = 0.;
    for branch in &r.branches {
        let id = branch.identity.strip_prefix("line:").unwrap();
        for (end, suffix, currents, powers) in [
            ("from", "fr", &branch.current_from_a, &branch.power_from_va),
            ("to", "to", &branch.current_to_a, &branch.power_to_va),
        ] {
            let bus = input["line"][id][format!("bus_{end}")].as_str().unwrap();
            for (k, t) in input["line"][id][format!("terminal_map_{end}")]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let terminal = t.as_str().unwrap();
                let e = &expected["line"][id][terminal];
                let cr = e[format!("cr_{suffix}")].as_f64().unwrap();
                let ci = e[format!("ci_{suffix}")].as_f64().unwrap();
                near(currents[k][0], cr, tolerance("current_a"));
                near(currents[k][1], ci, tolerance("current_a"));
                let v = &expected["bus"][bus][terminal];
                let vr = v["vr"].as_f64().unwrap();
                let vi = v["vi"].as_f64().unwrap();
                near(powers[k][0], vr * cr + vi * ci, tolerance("power_va"));
                near(powers[k][1], vi * cr - vr * ci, tolerance("power_va"));
                loss += powers[k][0];
                reference_loss += vr * cr + vi * ci;
            }
        }
    }
    near(loss, reference_loss, tolerance("power_va"));
    let device_power = |kind| {
        r.devices
            .iter()
            .filter(|d| d.kind == kind)
            .flat_map(|d| &d.coil_power_va)
            .map(|p| p[0])
            .sum::<f64>()
    };
    let tx_loss = device_power(McOpfDeviceKind::Source) + device_power(McOpfDeviceKind::Ibr)
        - device_power(McOpfDeviceKind::Load)
        - loss;
    near(
        tx_loss,
        expected["transformer"]["tx"]["loss"]["p_loss"]
            .as_f64()
            .unwrap(),
        tolerance("power_va"),
    );
    let inv = r.devices.iter().find(|d| d.identity == "inv").unwrap();
    for (k, t) in ["a", "b", "c"].iter().enumerate() {
        for (j, key) in ["pg", "qg"].iter().enumerate() {
            near(
                inv.coil_power_va[k][j],
                expected["ibr"]["inv"][t][key].as_f64().unwrap(),
                tolerance("power_va"),
            );
        }
        assert!(inv.coil_power_va[k][0] <= 40. + 1e-5);
        assert!(inv.coil_power_va[k][0] >= -1e-5);
    }
    assert!(r.residuals.maximum() <= 1e-6);
}
fn measurements(r: &McOpfResult) -> [f64; 2] {
    let mag = r.solution.terminal_voltage_magnitude("c", "a").unwrap();
    let angle = r.solution.terminal_voltage_angle("c", "a").unwrap();
    [r.solution.objective(), mag * angle.cos()]
}

#[test]
fn compact_feeder_reference_sensitivities_and_retained_study() {
    let timer = Instant::now();
    let n = network();
    let mut session =
        McOpfSession::new(McAcOpfInstance::from_network(n.clone()).unwrap(), options()).unwrap();
    let baseline = session.solve().unwrap().result.clone();
    reference(&baseline);
    let p = McOpfParameter {
        owner: McOpfParameterOwner::Load,
        component: "w".into(),
        axis: Some(0),
        kind: McOpfParameterKind::LoadActivePower,
    };
    let out = [
        McOpfSensitivityOutput::Objective,
        McOpfSensitivityOutput::Voltage {
            terminal: tellegen::mc_opf::McOpfTerminalId {
                bus: "c".into(),
                terminal: "a".into(),
            },
            part: McOpfVoltagePart::Real,
        },
    ];
    let duals = baseline.duals.as_ref().unwrap();
    assert_eq!(
        duals.status,
        McOpfMarginalStatus::Conditional,
        "{:?}",
        duals.active_rank
    );
    // One factorization serves all queries; only one physical direction is perturbed.
    let view = baseline.sensitivities().unwrap();
    assert!(view.diagnostics().kkt_dimension < 600);
    let (forward, diagnostic) = view
        .forward_with_diagnostics(
            &[McOpfParameterDirection {
                parameter: p.clone(),
                delta: 1.,
            }],
            &out,
        )
        .unwrap();
    assert!(diagnostic.residual_tolerance_ratio <= 1.);
    let weights = [
        McOpfOutputWeight {
            output: out[0].clone(),
            weight: 1.,
        },
        McOpfOutputWeight {
            output: out[1].clone(),
            weight: 0.01,
        },
    ];
    let adjoint = view.adjoint(&weights, std::slice::from_ref(&p)).unwrap();
    near(
        adjoint[0].derivative,
        forward[0].derivative + 0.01 * forward[1].derivative,
        1e-9,
    );
    let mut plus = n.clone();
    let mut minus = n;
    plus.loads_mut()
        .iter_mut()
        .find(|d| d.name == "w")
        .unwrap()
        .p_nom[0] += 0.1;
    minus
        .loads_mut()
        .iter_mut()
        .find(|d| d.name == "w")
        .unwrap()
        .p_nom[0] -= 0.1;
    let plus = measurements(&solve(plus));
    let minus = measurements(&solve(minus));
    for k in 0..2 {
        near(
            forward[k].derivative,
            (plus[k] - minus[k]) / 0.2,
            2e-7 + 5e-4 * forward[k].derivative.abs(),
        );
    }
    let marginal = duals
        .marginals
        .iter()
        .find(|m| m.parameter == p)
        .unwrap()
        .derivative
        .unwrap();
    near(forward[0].derivative, marginal, 1e-8);
    drop(view);
    session
        .apply_edits(&[
            McOpfEdit::LoadPower {
                load: "w".into(),
                p_w: vec![105., 145., 85.],
                q_var: vec![21., 31., 11.],
            },
            McOpfEdit::InverterAvailability {
                inverter: "inv".into(),
                p_max_w: Some(vec![35., 38., 36.]),
            },
            McOpfEdit::InverterControlProfile {
                profile: "cp".into(),
                power_factor: None,
                volt_var_breakpoints_v: None,
                volt_var_q_limits: Some(vec![-0.35, 0.35]),
                volt_watt_breakpoints_v: None,
                volt_watt_p_limits: None,
            },
        ])
        .unwrap();
    assert_eq!(
        session.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
    let warm = session.solve().unwrap().clone();
    assert_eq!(warm.start, McOpfStartUsed::PreviousPrimal);
    let cold = solve(session.instance().network().clone());
    for (a, b) in measurements(&warm.result)
        .into_iter()
        .zip(measurements(&cold))
    {
        near(a, b, 1e-6);
    }
    for device in &warm.result.devices {
        let other = cold
            .devices
            .iter()
            .find(|d| d.identity == device.identity && d.kind == device.kind)
            .unwrap();
        for (a, b) in device
            .coil_power_va
            .iter()
            .flatten()
            .zip(other.coil_power_va.iter().flatten())
        {
            near(*a, *b, 1e-5);
        }
    }
    for branch in &warm.result.branches {
        let other = cold
            .branches
            .iter()
            .find(|b| b.identity == branch.identity)
            .unwrap();
        for (a, b) in branch
            .current_from_a
            .iter()
            .chain(&branch.current_to_a)
            .flatten()
            .zip(
                other
                    .current_from_a
                    .iter()
                    .chain(&other.current_to_a)
                    .flatten(),
            )
        {
            near(*a, *b, 1e-6);
        }
    }
    assert_eq!(
        warm.result.duals.as_ref().unwrap().status,
        McOpfMarginalStatus::Conditional
    );
    assert!(warm.result.residuals.maximum() <= 1e-6);
    assert_eq!(session.solve_count(), 2);
    assert_eq!(session.compilation_count(), 2);
    let revision = session.revision();
    assert!(session
        .apply_edits(&[McOpfEdit::InverterAvailability {
            inverter: "missing".into(),
            p_max_w: Some(vec![1.])
        }])
        .is_err());
    assert_eq!(session.revision(), revision);
    assert!(session.current_result().is_some());
    eprintln!(
        "compact feeder: 5 solves, 1 factorization, {:.3}s",
        timer.elapsed().as_secs_f64()
    );
}
