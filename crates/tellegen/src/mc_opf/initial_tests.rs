use super::{
    initial, model,
    tests::{network, rich_network},
    *,
};
use num_complex::Complex64 as C;
use powerio::MulticonductorNetwork;
use powerio_prob::{MulticonductorOperatingPointBuilder as Builder, OperatingPoint};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8 * (1.0 + b.abs()), "{a} != {b}");
}
fn instance(net: MulticonductorNetwork) -> McAcOpfInstance {
    McAcOpfInstance::from_network(net).unwrap()
}
fn seeded(instance: &McAcOpfInstance, vb: f64, sb: f64) -> (model::Model, Vec<McOpfDiagnostic>) {
    let prep = initial::prepare(instance, &McAcOpfAssemblyOptions::new(vb, sb)).unwrap();
    let mut model = model::compile(prep).unwrap();
    let diagnostics = initial::apply(&mut model, instance).unwrap();
    (model, diagnostics)
}
fn phasor(pair: &model::Pair, x: &[f64]) -> C {
    C::new(pair[0].value(x), pair[1].value(x))
}
fn from_result(r: &McOpfResult) -> OperatingPoint<MulticonductorNetwork> {
    Builder::for_point(r.solution.instance().network().clone())
        .terminal_voltage_magnitudes(
            r.terminals
                .iter()
                .map(|v| C::new(v.voltage_v[0], v.voltage_v[1]).norm())
                .collect(),
        )
        .terminal_voltage_angles(
            r.terminals
                .iter()
                .map(|v| C::new(v.voltage_v[0], v.voltage_v[1]).arg())
                .collect(),
        )
        .build_point()
        .unwrap()
}
pub(super) fn case(bundle: &str, name: &str) -> MulticonductorNetwork {
    let text = match bundle {
        "components" => include_str!("../../tests/data/mc_opf/components.json"),
        "controls" => include_str!("../../tests/data/mc_opf/controls.json"),
        "bounds" => include_str!("../../tests/data/mc_opf/bounds.json"),
        _ => unreachable!(),
    };
    let cases: serde_json::Value = serde_json::from_str(text).unwrap();
    let module = powerio::parse_with_options(
        powerio::Source::from_memory("seed.bmopf.json", serde_json::to_vec(&cases[name]).unwrap())
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

#[test]
fn voltage_starts_follow_identity_and_units_not_storage_order() {
    let net = network();
    let point = Builder::for_point(net.clone())
        .terminal_voltage_magnitudes(vec![100., 0., 210., 2.])
        .terminal_voltage_angles(vec![0.5, 0., 0.2, -0.4])
        .build_point()
        .unwrap();
    let mut reordered = net;
    reordered.buses_mut().reverse();
    for b in reordered.buses_mut() {
        b.terminals.reverse();
    }
    let i = instance(reordered).with_initial_point(point);
    for (vb, sb) in [(230., 1000.), (100., 4000.)] {
        let (m, diagnostics) = seeded(&i, vb, sb);
        for (terminal, pair) in m.prep.terminals.iter().zip(&m.voltage) {
            let v = phasor(pair, &m.problem.x0) * vb;
            match (terminal.bus.as_str(), terminal.terminal.as_str()) {
                ("s", "a") => {
                    near(v.re, 230.);
                    near(v.im, 0.);
                }
                ("s", "n") => near(v.norm(), 0.),
                ("b", "a") => {
                    near(v.norm(), 210.);
                    near(v.arg(), 0.2);
                }
                ("b", "n") => {
                    near(v.norm(), 2.);
                    near(v.arg(), -0.4);
                }
                _ => unreachable!(),
            }
        }
        assert!(diagnostics
            .iter()
            .any(|d| d.target.as_deref() == Some("s/a")));
    }
    // Canonical serialization must also preserve keyed guesses when the point
    // was built against a different table order from the target instance.
    let solved = solve_mc_ac_opf_instance(Arc::new(i), &Default::default()).unwrap();
    let output = solved.to_module_json().unwrap();
    let report = McOpfResult::from_module_json(&output).unwrap();
    let point = report.solution.instance().initial_point().unwrap();
    near(point.terminal_voltage_magnitude("b", "a").unwrap(), 210.);
    near(point.terminal_voltage_angle("b", "n").unwrap(), -0.4);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "MC_OPF.INITIAL_POINT_PROJECTED"));
}

#[test]
fn initial_point_changes_only_x0_not_the_problem_or_selected_limits() {
    let net = rich_network();
    let mut constraints = powerio_prob::MulticonductorActiveConstraints::default();
    constraints.conductor_limits = powerio_prob::ConstraintSelection::None;
    constraints.generator_capability = powerio_prob::ConstraintSelection::Only(vec!["g".into()]);
    let cold = instance(net.clone())
        .with_objective(powerio_prob::Objective::default())
        .with_constraints(constraints);
    let point = Builder::for_point(net)
        .terminal_voltage_magnitudes(vec![230., 0., 210., 0.])
        .build_point()
        .unwrap();
    let warm = cold.clone().with_initial_point(point);
    let (a, _) = seeded(&cold, 230., 1000.);
    let (b, _) = seeded(&warm, 230., 1000.);
    assert_ne!(a.problem.x0, b.problem.x0);
    assert_eq!(
        result::fingerprint(&a.prep).unwrap(),
        result::fingerprint(&b.prep).unwrap()
    );
    assert_eq!(a.problem.var_names, b.problem.var_names);
    assert_eq!(a.problem.con_names, b.problem.con_names);
    assert_eq!(a.problem.g_l, b.problem.g_l);
    assert_eq!(a.problem.g_u, b.problem.g_u);
    assert_eq!(a.problem.x_l, b.problem.x_l);
    assert_eq!(a.problem.x_u, b.problem.x_u);
    let mut left = NlTnlp::try_new(a.problem.clone()).unwrap();
    let mut right = NlTnlp::try_new(b.problem.clone()).unwrap();
    for offset in [0.0, 0.03] {
        let x: Vec<_> = a.problem.x0.iter().map(|v| v + offset).collect();
        near(
            left.eval_f(&x, true).unwrap(),
            right.eval_f(&x, true).unwrap(),
        );
        let mut left_rows = vec![0.0; a.problem.m];
        let mut right_rows = vec![0.0; b.problem.m];
        assert!(left.eval_g(&x, true, &mut left_rows));
        assert!(right.eval_g(&x, true, &mut right_rows));
        assert_eq!(left_rows, right_rows);
    }
}

#[test]
fn terminal_power_guesses_initialize_currents_without_changing_loads_or_dispatch_bounds() {
    let net = rich_network();
    let point = Builder::for_point(net.clone())
        .load_active_powers(vec![180., 0.])
        .load_reactive_powers(vec![30., 0.])
        .generator_active_powers(vec![20., 0.])
        .build_point()
        .unwrap();
    let i = instance(net).with_initial_point(point);
    let (m, _) = seeded(&i, 230., 4000.);
    for (d, cols) in m.prep.devices.iter().zip(&m.devices) {
        if d.identity == "d" {
            near(m.problem.x0[cols.power[0][0]] * 4000., 180.);
            near(m.problem.x0[cols.power[0][1]] * 4000., 30.);
        }
        if d.identity == "g" {
            near(m.problem.x0[cols.power[0][0]] * 4000., 20.);
        }
    }
    let solved = solve_mc_ac_opf_instance(Arc::new(i), &Default::default()).unwrap();
    let load = solved.devices.iter().find(|d| d.identity == "d").unwrap();
    near(load.coil_power_va[0][0], 100.);
    let generator = solved.devices.iter().find(|d| d.identity == "g").unwrap();
    assert!(generator.coil_power_va[0][0] > 49.);
}

#[test]
fn delta_power_columns_are_terminal_quantities_not_coil_quantities() {
    let net = case("components", "capacitor_delta");
    let solved =
        solve_mc_ac_opf_instance(Arc::new(instance(net.clone())), &Default::default()).unwrap();
    let mut p = Vec::new();
    let mut q = Vec::new();
    for load in net.loads() {
        let d = solved
            .devices
            .iter()
            .find(|d| d.kind == powerio_matrix::McOpfDeviceKind::Load && d.identity == load.name)
            .unwrap();
        p.extend(d.terminal_power_va.iter().map(|v| v[0]));
        q.extend(d.terminal_power_va.iter().map(|v| v[1]));
    }
    let point = Builder::for_point(net.clone())
        .terminal_voltage_magnitudes(
            solved
                .terminals
                .iter()
                .map(|v| C::new(v.voltage_v[0], v.voltage_v[1]).norm())
                .collect(),
        )
        .terminal_voltage_angles(
            solved
                .terminals
                .iter()
                .map(|v| C::new(v.voltage_v[0], v.voltage_v[1]).arg())
                .collect(),
        )
        .load_active_powers(p)
        .load_reactive_powers(q)
        .build_point()
        .unwrap();
    let (m, _) = seeded(
        &instance(net).with_initial_point(point.clone()),
        230.,
        1000.,
    );
    for (d, cols) in m
        .prep
        .devices
        .iter()
        .zip(&m.devices)
        .filter(|(d, _)| d.kind == powerio_matrix::McOpfDeviceKind::Load)
    {
        for &t in &d.terminals {
            let current: C = d
                .coils
                .iter()
                .zip(&cols.current)
                .map(|(c, i)| {
                    (f64::from(c.positive == t) - f64::from(c.negative == Some(t)))
                        * phasor(i, &m.problem.x0)
                })
                .sum();
            let power = phasor(&m.voltage[t], &m.problem.x0) * current.conj() * 1000.;
            near(
                power.re,
                point
                    .load_active_power(&d.identity, &m.prep.terminals[t].terminal)
                    .unwrap(),
            );
            near(
                power.im,
                point
                    .load_reactive_power(&d.identity, &m.prep.terminals[t].terminal)
                    .unwrap(),
            );
        }
    }
}

#[test]
fn tap_starts_use_physical_and_inverse_regulator_conventions() {
    for name in ["tap_single_phase", "tap_regulator_a", "tap_regulator_b"] {
        let net = case("controls", name);
        let cold = instance(net.clone());
        let prep = initial::prepare(&cold, &Default::default()).unwrap();
        let tx = &prep.transformers[0];
        let [lo, hi] = tx.tap_bounds.unwrap();
        let alpha = (lo + hi) * 0.5;
        let physical = if tx.tap_inverse {
            tx.tap_nominal / alpha
        } else {
            tx.tap_nominal * alpha
        };
        let point = Builder::for_point(net)
            .transformer_taps(vec![physical])
            .build_point()
            .unwrap();
        let (m, _) = seeded(&cold.with_initial_point(point), 230., 1000.);
        near(m.problem.x0[m.transformer_tap[0].unwrap()], alpha);
    }
    let net = case("components", "transformer_single_phase");
    let cold = instance(net.clone());
    let point = Builder::for_point(net)
        .transformer_taps(vec![1.2])
        .build_point()
        .unwrap();
    let (a, _) = seeded(&cold, 230., 1000.);
    let (b, notes) = seeded(&cold.with_initial_point(point), 230., 1000.);
    assert!(b.transformer_tap[0].is_none());
    assert_eq!(
        result::fingerprint(&a.prep).unwrap(),
        result::fingerprint(&b.prep).unwrap()
    );
    assert!(notes
        .iter()
        .any(|d| d.target.as_deref() == Some(b.prep.transformers[0].identity.as_str())));
}

#[test]
fn all_lifted_starts_are_derived_from_the_seeded_phasors_and_currents() {
    let mut saw = [false; 4];
    for (bundle, name) in [
        ("components", "load_zip"),
        ("components", "ibr_droop_pn_per_phase"),
        ("bounds", "S2_vneg_max"),
    ] {
        let net = case(bundle, name);
        let result =
            solve_mc_ac_opf_instance(Arc::new(instance(net.clone())), &Default::default()).unwrap();
        let (m, _) = seeded(
            &instance(net).with_initial_point(from_result(&result)),
            100.,
            3000.,
        );
        for lift in &m.start_lifts {
            match lift {
                model::StartLift::Power(ids, u, i) => {
                    saw[0] = true;
                    let p = phasor(u, &m.problem.x0) * phasor(i, &m.problem.x0).conj();
                    near(m.problem.x0[ids[0]], p.re);
                    near(m.problem.x0[ids[1]], p.im);
                }
                model::StartLift::Magnitude(id, u) => {
                    saw[1] = true;
                    near(m.problem.x0[*id], phasor(u, &m.problem.x0).norm());
                }
                model::StartLift::LogVoltage(id, u, n) => {
                    saw[2] = true;
                    near(
                        m.problem.x0[*id],
                        (phasor(u, &m.problem.x0).norm() / n).ln(),
                    );
                }
                model::StartLift::Voltage(to, from) => {
                    saw[3] = true;
                    near(
                        (phasor(to, &m.problem.x0) - phasor(from, &m.problem.x0)).norm(),
                        0.,
                    );
                }
            }
        }
    }
    assert!(saw.into_iter().all(|b| b));
}

#[test]
fn zero_rated_and_singular_equipment_keep_finite_consistent_seed_coordinates() {
    let mut net = network();
    net.loads_mut().clear();
    net.lines_mut()[0].i_max = Some(vec![0., 0.]);
    let point = Builder::for_point(net.clone())
        .terminal_voltage_magnitudes(vec![230., 0., 150., 0.])
        .build_point()
        .unwrap();
    let i = instance(net).with_initial_point(point);
    let (m, _) = seeded(&i, 230., 1000.);
    assert!(!m.zero_columns.is_empty());
    for &id in &m.zero_columns {
        assert_eq!(m.problem.x0[id], 0.);
    }
    solve_mc_ac_opf_instance(Arc::new(i), &Default::default()).unwrap();
    let mut net = network();
    net.line_codes_mut()[0].r_series = vec![vec![0.; 2]; 2];
    let point = Builder::for_point(net.clone())
        .terminal_voltage_magnitudes(vec![230., 0., 150., 0.])
        .build_point()
        .unwrap();
    let i = instance(net).with_initial_point(point);
    let (m, _) = seeded(&i, 230., 1000.);
    assert!(m.problem.x0.iter().all(|v| v.is_finite()));
    solve_mc_ac_opf_instance(Arc::new(i), &Default::default()).unwrap();
}

#[test]
fn a_poor_partial_voltage_start_can_solve_and_roundtrip_without_changing_the_model() {
    let net = network();
    let cold = instance(net.clone());
    let point = Builder::for_point(net)
        .terminal_voltage_magnitudes(vec![230., 0., 150., 0.])
        .build_point()
        .unwrap();
    let warm = cold.clone().with_initial_point(point);
    let baseline = solve_mc_ac_opf_instance(Arc::new(cold), &Default::default()).unwrap();
    let input =
        crate::ir::serialize_module(&PioModule::new(PioValue::McAcOpfInstance(warm))).unwrap();
    let output = solve_mc_ac_opf_module_json(&input, &Default::default()).unwrap();
    let result = McOpfResult::from_module_json(&output).unwrap();
    near(result.solution.objective(), baseline.solution.objective());
    assert_eq!(result.fingerprint, baseline.fingerprint);
    near(
        result
            .solution
            .instance()
            .initial_point()
            .unwrap()
            .terminal_voltage_magnitude("b", "a")
            .unwrap(),
        150.,
    );
    McOpfResult::from_module_json(&result.to_module_json().unwrap()).unwrap();
}

#[test]
fn invalid_mismatched_or_discrete_seed_fields_are_not_silently_ignored() {
    for bad in [f64::NAN, f64::INFINITY, -1.] {
        let net = network();
        let point = Builder::for_point(net.clone())
            .terminal_voltage_magnitudes(vec![230., 0., bad, 0.])
            .build_point()
            .unwrap();
        let error = solve_mc_ac_opf_instance(
            Arc::new(instance(net).with_initial_point(point)),
            &Default::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
        assert!(error.diagnostics[0]
            .target
            .as_ref()
            .unwrap()
            .contains("b/a"));
    }
    let net = network();
    let mut other = net.clone();
    other.buses_mut()[1].terminals[0] = "wrong".into();
    let point = Builder::for_point(other)
        .terminal_voltage_angles(vec![0.; 4])
        .build_point()
        .unwrap();
    assert_eq!(
        initial::prepare(
            &instance(net.clone()).with_initial_point(point),
            &Default::default()
        )
        .unwrap_err()
        .kind,
        McOpfErrorKind::InvalidInput
    );
    let point = Builder::for_point(net.clone())
        .switch_closed(vec![])
        .build_point()
        .unwrap();
    assert_eq!(
        initial::prepare(
            &instance(net).with_initial_point(point),
            &Default::default()
        )
        .unwrap_err()
        .kind,
        McOpfErrorKind::UnsupportedInput
    );
}

#[test]
fn inconsistent_terminal_power_and_zero_log_voltage_starts_are_diagnosed() {
    let net = network();
    let point = Builder::for_point(net.clone())
        .load_active_powers(vec![100., 10.])
        .build_point()
        .unwrap();
    let i = instance(net).with_initial_point(point);
    let error = solve_mc_ac_opf_instance(Arc::new(i), &Default::default()).unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
    assert_eq!(error.diagnostics[0].target.as_deref(), Some("d"));
    let net = case("components", "load_zip");
    let n = net.buses().iter().map(|b| b.terminals.len()).sum();
    let point = Builder::for_point(net.clone())
        .terminal_voltage_magnitudes(vec![0.; n])
        .build_point()
        .unwrap();
    let error = solve_mc_ac_opf_instance(
        Arc::new(instance(net).with_initial_point(point)),
        &Default::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
    assert!(error.diagnostics[0]
        .target
        .as_ref()
        .unwrap()
        .contains("log_voltage"));
}

#[cfg(feature = "mc-pf")]
#[test]
fn a_converged_pf_solution_supplies_a_voltage_seed_for_opf() {
    let net = network();
    let pf = powerio::McAcPfInstance::from_network(net.clone()).unwrap();
    let solved = crate::solve_mc_ac_pf_instance(&pf, &Default::default()).unwrap();
    let solution = solved.to_powerio_solution(&pf).unwrap();
    let point = mc_opf_initial_point_from_pf(&solution).unwrap();
    let opf = solve_mc_ac_opf_instance(
        Arc::new(instance(net).with_initial_point(point)),
        &Default::default(),
    )
    .unwrap();
    near(
        opf.solution.terminal_voltage_magnitude("b", "a").unwrap(),
        solution.terminal_voltage_magnitude("b", "a").unwrap(),
    );
}

#[test]
fn pf_seed_helper_rejects_unconverged_or_nonfinite_solutions() {
    let pf = Arc::new(powerio::McAcPfInstance::from_network(network()).unwrap());
    for (termination, magnitude) in [
        (Termination::IterationLimit, 230.),
        (Termination::NotReported, 230.),
        (Termination::Converged, f64::NAN),
    ] {
        let solution = powerio::McAcPfSolution::new(
            pf.clone(),
            termination,
            vec![230., 0., magnitude, 0.],
            vec![0.; 4],
            vec![0.; 2],
        )
        .unwrap();
        let error = mc_opf_initial_point_from_pf(&solution).unwrap_err();
        assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
        assert_eq!(error.diagnostics[0].code, "MC_OPF.INITIAL_POINT_INVALID");
    }
}
