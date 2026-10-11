use super::model::Model;
use super::*;
use pounce_rs::SparsityRequest;
use powerio_dist::{
    Configuration, DistBus, DistGenerator, DistLine, DistLineCode, DistLoad, DistShunt, DistSwitch,
    MulticonductorNetwork, VoltageSource,
};
use powerio_prob::ConstraintSelection;
fn names(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| (*s).into()).collect()
}
fn network() -> MulticonductorNetwork {
    let mut n = MulticonductorNetwork::new();
    let mut source = DistBus::new("s", names(&["a", "n"]));
    source.grounded = names(&["n"]);
    n.buses_mut().push(source);
    let mut bus = DistBus::new("b", names(&["a", "n"]));
    bus.vpn_min = Some(vec![200.]);
    bus.vpn_max = Some(vec![250.]);
    bus.vn_max = Some(10.);
    n.buses_mut().push(bus);
    n.line_codes_mut().push(DistLineCode::new(
        "c",
        vec![vec![0.1, 0.], vec![0., 0.1]],
        vec![vec![0.; 2]; 2],
    ));
    n.lines_mut().push(DistLine::new(
        "l",
        "s",
        "b",
        names(&["a", "n"]),
        names(&["a", "n"]),
        "c",
        1.,
    ));
    n.loads_mut().push(DistLoad::new(
        "d",
        "b",
        names(&["a", "n"]),
        Configuration::Wye,
        vec![100.],
        vec![0.],
    ));
    let mut grid = VoltageSource::new(
        "grid",
        "s",
        names(&["a", "n"]),
        vec![230., 0.],
        vec![0., 0.],
    );
    grid.energy_cost_rate = Some(vec![0.2]);
    n.sources_mut().push(grid);
    n
}
fn instance(n: MulticonductorNetwork) -> Arc<McAcOpfInstance> {
    Arc::new(McAcOpfInstance::from_network(n).unwrap())
}
fn compiled(n: MulticonductorNetwork) -> Model {
    model::compile(build_mc_ac_opf_preparation(&instance(n), &Default::default()).unwrap()).unwrap()
}
fn solve(n: MulticonductorNetwork) -> McOpfResult {
    solve_mc_ac_opf_instance(instance(n), &Default::default()).unwrap()
}
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} (tol {tol})");
}
fn rich_network() -> MulticonductorNetwork {
    let mut n = network();
    let c = &mut n.line_codes_mut()[0];
    c.r_series[0][1] = 0.02;
    c.r_series[1][0] = 0.02;
    c.x_series = vec![vec![0.04, 0.01], vec![0.01, 0.04]];
    c.g_from = vec![vec![1e-5, -2e-6], vec![-2e-6, 1e-5]];
    c.b_to = vec![vec![2e-5, -3e-6], vec![-3e-6, 2e-5]];
    c.i_max = Some(vec![20., 20.]);
    c.s_max = Some(vec![4000., 1000.]);
    let mut g = DistGenerator::new(
        "g",
        "b",
        names(&["a", "n"]),
        Configuration::Wye,
        vec![0.],
        vec![0.],
    );
    g.p_min = Some(vec![0.]);
    g.p_max = Some(vec![50.]);
    g.q_min = Some(vec![0.]);
    g.q_max = Some(vec![0.]);
    g.s_max = Some(vec![80.]);
    g.i_max = Some(vec![2., 3.]);
    g.cost = Some(vec![0.05]);
    n.generators_mut().push(g);
    n.shunts_mut().push(DistShunt::new(
        "sh",
        "b",
        names(&["a", "n"]),
        vec![vec![1e-5, -1e-5], vec![-1e-5, 1e-5]],
        vec![vec![0.; 2]; 2],
    ));
    n
}
#[test]
fn resistive_two_wire_solution_matches_closed_form() {
    let r = solve(network());
    let u = (230. + (230_f64.powi(2) - 4. * 0.2 * 100.).sqrt()) / 2.;
    let i = 100. / u;
    near(r.branches[0].current_from_a[0][0], i, 1e-6);
    near(r.branches[0].current_from_a[1][0], -i, 1e-6);
    near(
        r.solution.terminal_voltage_magnitude("b", "n").unwrap(),
        0.1 * i,
        1e-6,
    );
    near(
        r.solution.objective(),
        0.2 * (100. + 0.2 * i * i) / 1000.,
        1e-8,
    );
    assert!(r.residuals.maximum() < 1e-7);
}
#[test]
fn cheap_generator_dispatch_and_terminal_neutral_projection() {
    let r = solve(rich_network());
    let g = r
        .devices
        .iter()
        .find(|d| d.kind == powerio_matrix::McOpfDeviceKind::Generator)
        .unwrap();
    near(g.coil_power_va[0][0], 50., 2e-3);
    near(
        g.terminal_power_va.iter().map(|p| p[0]).sum(),
        g.coil_power_va[0][0],
        1e-8,
    );
    assert_eq!(r.solution.generator_active_powers().len(), 2);
}
#[test]
fn equation_rows_match_independent_complex_arithmetic_away_from_solution() {
    let m = compiled(rich_network());
    let x: Vec<_> = m
        .problem
        .x0
        .iter()
        .enumerate()
        .map(|(i, v)| v + 0.13 * ((i + 1) as f64).sin())
        .collect();
    let mut t = NlTnlp::try_new(m.problem.clone()).unwrap();
    let mut g = vec![0.; m.problem.m];
    assert!(t.eval_g(&x, true, &mut g));
    let voltage: Vec<_> = m
        .voltage
        .iter()
        .map(|v| num_complex::Complex64::new(v[0].value(&x), v[1].value(&x)))
        .collect();
    let b = &m.prep.branches[0];
    let cur: Vec<_> = m.branch_current[0]
        .iter()
        .map(|v| num_complex::Complex64::new(v[0].value(&x), v[1].value(&x)))
        .collect();
    for k in 0..2 {
        let z: num_complex::Complex64 = cur
            .iter()
            .enumerate()
            .map(|(j, i)| num_complex::Complex64::new(b.r[k][j], b.x[k][j]) * i)
            .sum();
        let e = voltage[b.from[k]] - voltage[b.to[k]] - z;
        for (p, v) in [e.re, e.im].iter().enumerate() {
            let row = m
                .problem
                .con_names
                .iter()
                .position(|s| s == &format!("line:l:{k}:kvl:{p}"))
                .unwrap();
            near(g[row], *v, 1e-12);
        }
    }
    for (d, cols) in m.prep.devices.iter().zip(&m.devices) {
        for (k, c) in d.coils.iter().enumerate() {
            let u = voltage[c.positive]
                - c.negative
                    .map_or(num_complex::Complex64::new(0., 0.), |n| voltage[n]);
            let i = num_complex::Complex64::new(
                cols.current[k][0].value(&x),
                cols.current[k][1].value(&x),
            );
            let p = u * i.conj();
            for (j, v) in [p.re, p.im].iter().enumerate() {
                let row = m
                    .problem
                    .con_names
                    .iter()
                    .position(|s| s == &format!("{:?}:{}:{k}:power:{j}", d.kind, d.identity))
                    .unwrap();
                near(g[row], x[cols.power[k][j]] - v, 1e-12);
            }
        }
    }
}
fn derivatives(
    t: &mut NlTnlp,
    x: &[f64],
    lambda: &[f64],
    sigma: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let info = t.get_nlp_info().unwrap();
    let n = x.len();
    let mut grad = vec![0.; n];
    assert!(t.eval_grad_f(x, true, &mut grad));
    let mut jr = vec![0; info.nnz_jac_g as usize];
    let mut jc = jr.clone();
    assert!(t.eval_jac_g(
        None,
        false,
        SparsityRequest::Structure {
            irow: &mut jr,
            jcol: &mut jc
        }
    ));
    let mut j = vec![0.; jr.len()];
    assert!(t.eval_jac_g(Some(x), true, SparsityRequest::Values { values: &mut j }));
    let mut dense = vec![0.; lambda.len() * n];
    for ((r, c), v) in jr.into_iter().zip(jc).zip(j) {
        dense[r as usize * n + c as usize] += v;
    }
    let mut hr = vec![0; info.nnz_h_lag as usize];
    let mut hc = hr.clone();
    assert!(t.eval_h(
        None,
        false,
        sigma,
        None,
        false,
        SparsityRequest::Structure {
            irow: &mut hr,
            jcol: &mut hc
        }
    ));
    let mut h = vec![0.; hr.len()];
    assert!(t.eval_h(
        Some(x),
        true,
        sigma,
        Some(lambda),
        true,
        SparsityRequest::Values { values: &mut h }
    ));
    let mut hd = vec![0.; n * n];
    for ((r, c), v) in hr.into_iter().zip(hc).zip(h) {
        hd[r as usize * n + c as usize] += v;
        if r != c {
            hd[c as usize * n + r as usize] += v;
        }
    }
    (grad, dense, hd)
}
fn lag_gradient(t: &mut NlTnlp, x: &[f64], lambda: &[f64], sigma: f64) -> Vec<f64> {
    let (g, j, _) = derivatives(t, x, lambda, sigma);
    (0..x.len())
        .map(|k| {
            sigma * g[k]
                + lambda
                    .iter()
                    .enumerate()
                    .map(|(r, l)| l * j[r * x.len() + k])
                    .sum::<f64>()
        })
        .collect()
}
#[test]
fn quadratic_classification_exact_derivatives_and_weighted_hessian() {
    let m = compiled(rich_network());
    let mut fast = NlTnlp::try_new_with_quadratic(m.problem.clone(), true).unwrap();
    let mut ad = NlTnlp::try_new_with_quadratic(m.problem.clone(), false).unwrap();
    assert!(fast.quadratic_objective());
    for r in 0..m.problem.m {
        assert!(
            fast.quadratic_row(r),
            "row {} not recognized",
            m.problem.con_names[r]
        );
    }
    let n = m.problem.n;
    let dir: Vec<_> = (0..n).map(|k| ((k * 7 + 3) as f64).sin()).collect();
    for point in 0..4 {
        let x: Vec<_> = m
            .problem
            .x0
            .iter()
            .enumerate()
            .map(|(k, v)| v + 0.1 * ((k + point * 3) as f64).cos())
            .collect();
        let lambda: Vec<_> = (0..m.problem.m)
            .map(|k| ((k + point) as f64).cos())
            .collect();
        let sigma = 0.3 + point as f64;
        let (a, j, h) = derivatives(&mut fast, &x, &lambda, sigma);
        let (b, jb, hb) = derivatives(&mut ad, &x, &lambda, sigma);
        for (u, v) in a
            .iter()
            .chain(&j)
            .chain(&h)
            .zip(b.iter().chain(&jb).chain(&hb))
        {
            near(*u, *v, 1e-9 * (1. + u.abs()));
        }
        for step in [1e-4, 1e-5, 1e-6] {
            let xp: Vec<_> = x.iter().zip(&dir).map(|(x, d)| x + step * d).collect();
            let xm: Vec<_> = x.iter().zip(&dir).map(|(x, d)| x - step * d).collect();
            let mut gp = vec![0.; lambda.len()];
            let mut gm = gp.clone();
            assert!(fast.eval_g(&xp, true, &mut gp));
            assert!(fast.eval_g(&xm, true, &mut gm));
            for r in 0..lambda.len() {
                let exact: f64 = (0..n).map(|k| j[r * n + k] * dir[k]).sum();
                near(
                    (gp[r] - gm[r]) / (2. * step),
                    exact,
                    2e-6 * (1. + exact.abs()),
                );
            }
            let lp = lag_gradient(&mut fast, &xp, &lambda, sigma);
            let lm = lag_gradient(&mut fast, &xm, &lambda, sigma);
            for r in 0..n {
                let exact: f64 = (0..n).map(|k| h[r * n + k] * dir[k]).sum();
                near(
                    (lp[r] - lm[r]) / (2. * step),
                    exact,
                    2e-6 * (1. + exact.abs()),
                );
            }
        }
    }
}
#[test]
fn zero_caps_compile_to_components_and_open_switch_has_no_variables() {
    let mut n = network();
    n.lines_mut()[0].i_max = Some(vec![0., 0.]);
    n.switches_mut().push(DistSwitch::new(
        "open",
        "s",
        "b",
        names(&["a"]),
        names(&["a"]),
        true,
    ));
    let m = compiled(n);
    assert!(m
        .problem
        .con_names
        .iter()
        .any(|s| s.contains("current:0:zero")));
    assert!(!m
        .problem
        .var_names
        .iter()
        .any(|s| s.contains("switch:open")));
}
#[test]
fn exact_ideal_line_and_delta_load_solve_without_impedance_inverse() {
    let mut n = network();
    n.line_codes_mut()[0].r_series = vec![vec![0.; 2]; 2];
    n.loads_mut()[0].configuration = Configuration::Delta;
    let r = solve(n);
    near(
        r.solution.terminal_voltage_magnitude("b", "a").unwrap(),
        230.,
        1e-6,
    );
    near(r.solution.objective(), 0.02, 1e-8);
}

#[test]
fn singular_nonzero_impedance_retains_a_well_defined_ivr_solution() {
    let mut n = network();
    n.line_codes_mut()[0].r_series = vec![vec![0.1; 2]; 2];
    let r = solve(n);
    // The equal and opposite phase/return currents lie in Z's nullspace.
    near(
        r.solution.terminal_voltage_magnitude("b", "a").unwrap(),
        230.0,
        1e-6,
    );
    near(r.branches[0].current_from_a[0][0], 100.0 / 230.0, 1e-6);
    near(r.solution.objective(), 0.02, 1e-8);
}
#[test]
fn independent_validator_rejects_corrupted_primal_auxiliary_objective_and_nonfinite() {
    let m = compiled(rich_network());
    let t = Rc::new(RefCell::new(NlTnlp::try_new(m.problem.clone()).unwrap()));
    let mut app = IpoptApplication::new();
    app.initialize_with_options_str("linear_solver feral\nprint_level 0\ntol 1e-9\n")
        .unwrap();
    app.initialize().unwrap();
    assert_eq!(
        app.optimize_tnlp(t.clone()),
        ApplicationReturnStatus::SolveSucceeded
    );
    let t = t.borrow();
    let x = t.final_x().unwrap().to_vec();
    let f = t.final_obj();
    validate::check(&m, &x, f, 1e-6).unwrap();
    for index in [
        0,
        m.devices[0].power[0][0],
        m.problem
            .var_names
            .iter()
            .position(|s| s == "line:l:from:0:p")
            .unwrap(),
    ] {
        let mut bad = x.clone();
        bad[index] += 0.1;
        assert!(validate::check(&m, &bad, f, 1e-6).is_err());
    }
    assert!(validate::check(&m, &x, f + 1., 1e-6).is_err());
    for v in [f64::NAN, f64::INFINITY] {
        let mut bad = x.clone();
        bad[0] = v;
        assert!(validate::check(&m, &bad, f, 1e-6).is_err());
        assert!(validate::check(&m, &x, v, 1e-6).is_err());
    }
}
#[test]
fn physical_results_are_invariant_to_working_base_and_bus_order() {
    let n = network();
    let a = solve(n.clone());
    let opts = McOpfOptions {
        voltage_base_v: 100.,
        power_base_va: 5000.,
        ..Default::default()
    };
    let b = solve_mc_ac_opf_instance(instance(n.clone()), &opts).unwrap();
    near(a.solution.objective(), b.solution.objective(), 1e-8);
    near(
        a.branches[0].current_from_a[0][0],
        b.branches[0].current_from_a[0][0],
        1e-5,
    );
    let mut reordered = n;
    reordered.buses_mut().reverse();
    let c = solve(reordered);
    near(
        a.solution.terminal_voltage_magnitude("b", "a").unwrap(),
        c.solution.terminal_voltage_magnitude("b", "a").unwrap(),
        1e-6,
    );
}
#[test]
fn canonical_module_roundtrip_retains_instance_and_terminal_axes() {
    let input = crate::ir::serialize_module(&PioModule::new(PioValue::McAcOpfInstance(
        (*instance(network())).clone(),
    )))
    .unwrap();
    let output = solve_mc_ac_opf_module_json(&input, &Default::default()).unwrap();
    let parsed = crate::ir::deserialize_module(&output).unwrap();
    let PioValue::McAcOpfSolution(s) = parsed.into_value() else {
        panic!("wrong result type")
    };
    assert_eq!(s.instance().network().loads()[0].name, "d");
    assert!(s.terminal_voltage_magnitude("b", "n").unwrap() > 0.);
    near(s.objective(), solve(network()).solution.objective(), 1e-10);
    let lossy = PioModule::new(PioValue::McAcOpfInstance((*instance(network())).clone()))
        .with_diagnostic(powerio::Diagnostic::new(
            powerio::DiagnosticCode::new("PARSE.TEST.FIELD_DROPPED").unwrap(),
            powerio::DiagnosticSeverity::Warning,
            "a physical field was dropped upstream",
        ))
        .unwrap();
    let input = crate::ir::serialize_module(&lossy).unwrap();
    assert!(
        solve_mc_ac_opf_module_json(&input, &McOpfOptions::default())
            .unwrap_err()
            .contains("diagnostic")
    );
}
#[test]
fn cancellation_and_invalid_options_stop_before_solver() {
    assert!(solve_mc_ac_opf_instance_cancellable(
        instance(network()),
        &Default::default(),
        Some(Arc::new(AtomicBool::new(true)))
    )
    .unwrap_err()
    .contains("cancelled"));
    assert!(solve_mc_ac_opf_instance(
        instance(network()),
        &McOpfOptions {
            acceptance_tolerance: f64::NAN,
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn infeasible_limits_never_produce_a_portable_solution() {
    let mut n = network();
    n.lines_mut()[0].i_max = Some(vec![0.1, 0.1]);
    assert!(solve_mc_ac_opf_instance(
        instance(n),
        &McOpfOptions {
            max_iterations: 100,
            ..Default::default()
        }
    )
    .is_err());
}
#[test]
fn constraint_selections_really_remove_limits() {
    let mut n = network();
    n.lines_mut()[0].i_max = Some(vec![0.1, 0.1]);
    let i = McAcOpfInstance::from_network(n).unwrap();
    let mut c = i.constraints().clone();
    c.conductor_limits = ConstraintSelection::None;
    let r = solve_mc_ac_opf_instance(Arc::new(i.with_constraints(c)), &Default::default()).unwrap();
    assert!(r.branches[0].current_from_a[0][0] > 0.1);
}

#[test]
fn component_jacobians_and_lagrangian_hessians_match_ad_and_differences() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/data/mc_opf/components.json")).unwrap();
    let controls: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/data/mc_opf/controls.json")).unwrap();
    let bounds: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/data/mc_opf/bounds.json")).unwrap();
    let cases: serde_json::Map<String, serde_json::Value> = cases
        .as_object()
        .unwrap()
        .iter()
        .chain(controls.as_object().unwrap())
        .chain(bounds.as_object().unwrap())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for name in [
        "tap_single_phase",
        "tap_center_tap",
        "tap_regulator_a",
        "tap_regulator_b",
        "tap_delta_wye",
        "tap_wye_delta",
        "tap_open_delta_caba",
        "S1_vpos_max",
        "S2_vneg_max",
        "S3_vzero_max",
        "D1_va_diff",
        "load_constant_current",
        "load_constant_impedance",
        "load_exponential",
        "load_zip",
        "ibr_droop_pn_per_phase",
        "ibr_pf_lag",
        "transformer_center_tap",
        "transformer_delta_wye",
        "transformer_n_winding",
        "transformer_regulator_a",
    ] {
        let module = powerio::parse_with_options(
            powerio::Source::from_memory(
                "derivatives.bmopf.json",
                serde_json::to_vec(&cases[name]).unwrap(),
            )
            .unwrap(),
            &powerio::ParseOptions::default()
                .format("bmopf-json")
                .unwrap(),
        )
        .unwrap();
        let powerio::PioValue::MulticonductorNetwork(net) = module.into_value() else {
            panic!("expected network")
        };
        let m = compiled(net);
        let mut fast = NlTnlp::try_new_with_quadratic(m.problem.clone(), true).unwrap();
        let mut ad = NlTnlp::try_new_with_quadratic(m.problem.clone(), false).unwrap();
        assert!(fast.quadratic_objective());
        if name.contains("transformer")
            || name.starts_with("tap_")
            || name.starts_with("S")
            || name.starts_with("D1")
            || name == "load_constant_impedance"
            || name == "ibr_pf_lag"
        {
            assert!(
                (0..m.problem.m).all(|r| fast.quadratic_row(r)),
                "{name}: polynomial row escaped quadratic path: {:?}",
                (0..m.problem.m)
                    .filter(|&r| !fast.quadratic_row(r))
                    .map(|r| &m.problem.con_names[r])
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(
                (0..m.problem.m).any(|r| !fast.quadratic_row(r)),
                "{name}: expected AD fallback"
            );
        }
        let n = m.problem.n;
        let dir: Vec<_> = (0..n).map(|k| ((k * 7 + 3) as f64).sin()).collect();
        for point in 0..3 {
            let x: Vec<_> = m
                .problem
                .x0
                .iter()
                .enumerate()
                .map(|(k, v)| v + 0.1 * ((k + point * 3) as f64).cos())
                .collect();
            let lambda: Vec<_> = (0..m.problem.m)
                .map(|k| ((k + point) as f64).cos())
                .collect();
            let sigma = 0.3 + point as f64;
            let (a, j, h) = derivatives(&mut fast, &x, &lambda, sigma);
            let (b, jb, hb) = derivatives(&mut ad, &x, &lambda, sigma);
            for (u, v) in a
                .iter()
                .chain(&j)
                .chain(&h)
                .zip(b.iter().chain(&jb).chain(&hb))
            {
                near(*u, *v, 1e-9 * (1. + u.abs()));
            }
            for step in [1e-5, 3e-6, 1e-6] {
                let xp: Vec<_> = x.iter().zip(&dir).map(|(x, d)| x + step * d).collect();
                let xm: Vec<_> = x.iter().zip(&dir).map(|(x, d)| x - step * d).collect();
                let mut gp = vec![0.; lambda.len()];
                let mut gm = gp.clone();
                assert!(fast.eval_g(&xp, true, &mut gp));
                assert!(fast.eval_g(&xm, true, &mut gm));
                for r in 0..lambda.len() {
                    let exact: f64 = (0..n).map(|k| j[r * n + k] * dir[k]).sum();
                    near(
                        (gp[r] - gm[r]) / (2. * step),
                        exact,
                        2e-4 * (1. + exact.abs()),
                    );
                }
                let lp = lag_gradient(&mut fast, &xp, &lambda, sigma);
                let lm = lag_gradient(&mut fast, &xm, &lambda, sigma);
                for r in 0..n {
                    let exact: f64 = (0..n).map(|k| h[r * n + k] * dir[k]).sum();
                    near(
                        (lp[r] - lm[r]) / (2. * step),
                        exact,
                        2e-4 * (1. + exact.abs()),
                    );
                }
            }
        }
    }
}

#[test]
fn impedance_load_remains_a_current_law_at_zero_voltage() {
    let mut n = network();
    n.sources_mut()[0].v_magnitude[0] = 0.0;
    n.buses_mut()[1].vpn_min = None;
    n.buses_mut()[1].vpn_max = None;
    n.loads_mut()[0].voltage_model =
        powerio_dist::DistLoadVoltageModel::ConstantImpedance { v_nom: vec![230.0] };
    let result = solve(n.clone());
    near(result.solution.objective(), 0.0, 1e-10);
    let load = result
        .devices
        .iter()
        .find(|d| d.kind == powerio_matrix::McOpfDeviceKind::Load)
        .unwrap();
    assert!(load.coil_current_a.iter().flatten().all(|v| v.abs() < 1e-8));
    // The same zero-voltage network cannot supply a nonzero constant-power load.
    n.loads_mut()[0].voltage_model =
        powerio_dist::DistLoadVoltageModel::ConstantPower { v_nom: Vec::new() };
    assert!(solve_mc_ac_opf_instance(instance(n), &Default::default()).is_err());
}

#[test]
fn zero_inverter_ratings_solve_without_duplicate_capability_equalities() {
    use powerio_dist::{DistIbr, IbrPrimeMover, IbrTopology};
    for current_zero in [false, true] {
        let mut n = network();
        let mut inv = DistIbr::new(
            "off",
            "b",
            names(&["a", "n"]),
            IbrTopology::SinglePhase,
            IbrPrimeMover::Pv,
            vec![if current_zero { 80.0 } else { 0.0 }],
        );
        inv.p_min = Some(vec![0.0]);
        inv.p_max = Some(vec![50.0]);
        inv.q_min = Some(vec![0.0]);
        inv.q_max = Some(vec![0.0]);
        if current_zero {
            inv.i_max = Some(vec![0.0, 0.0]);
        }
        inv.extras.insert("dc_link_coupled".into(), true.into());
        n.ibrs_mut().push(inv);
        let result = solve(n);
        let dev = result.devices.iter().find(|d| d.identity == "off").unwrap();
        assert!(dev.coil_power_va.iter().flatten().all(|v| v.abs() < 1e-6));
    }
}

#[test]
fn smooth_control_breakpoint_has_exact_first_and_second_derivatives() {
    use pounce_nl::nl_reader::{Expr, NlProblem, NlProblemParts};
    let problem = NlProblem::from_expressions(NlProblemParts {
        minimize: true,
        objective: model::softplus(Expr::Var(0)),
        obj_constant: 0.0,
        constraints: Vec::new(),
        x_l: vec![f64::NEG_INFINITY],
        x_u: vec![f64::INFINITY],
        x0: vec![0.0],
        g_l: Vec::new(),
        g_u: Vec::new(),
        var_names: vec!["z".into()],
        con_names: Vec::new(),
    })
    .unwrap();
    let mut t = NlTnlp::try_new(problem).unwrap();
    for (x, value, first, second) in [
        (0.0, 2.0_f64.ln(), 0.5, 0.25),
        (-1000.0, 0.0, 0.0, 0.0),
        (1000.0, 1000.0, 1.0, 0.0),
    ] {
        let f = t.eval_f(&[x], true).unwrap();
        near(f, value, 1e-12);
        let (gradient, _, hessian) = derivatives(&mut t, &[x], &[], 1.0);
        near(gradient[0], first, 1e-12);
        near(hessian[0], second, 1e-12);
    }
}

#[test]
fn independent_validation_detects_transformer_tap_and_inverter_corruption() {
    for (input, name) in [
        (
            include_str!("../../tests/data/mc_opf/components.json"),
            "transformer_center_tap",
        ),
        (
            include_str!("../../tests/data/mc_opf/controls.json"),
            "tap_regulator_b",
        ),
        (
            include_str!("../../tests/data/mc_opf/components.json"),
            "ibr_pf_lag",
        ),
    ] {
        let cases: serde_json::Value = serde_json::from_str(input).unwrap();
        let module = powerio::parse_with_options(
            powerio::Source::from_memory(
                "corruption.bmopf.json",
                serde_json::to_vec(&cases[name]).unwrap(),
            )
            .unwrap(),
            &powerio::ParseOptions::default()
                .format("bmopf-json")
                .unwrap(),
        )
        .unwrap();
        let PioValue::MulticonductorNetwork(net) = module.into_value() else {
            panic!("network")
        };
        let m = compiled(net);
        let t = Rc::new(RefCell::new(NlTnlp::try_new(m.problem.clone()).unwrap()));
        let mut app = IpoptApplication::new();
        app.initialize_with_options_str("linear_solver feral\nprint_level 0\ntol 1e-9\n")
            .unwrap();
        app.initialize().unwrap();
        assert_eq!(
            app.optimize_tnlp(t.clone()),
            ApplicationReturnStatus::SolveSucceeded,
            "{name}"
        );
        let t = t.borrow();
        let x = t.final_x().unwrap();
        let f = t.final_obj();
        validate::check(&m, x, f, 1e-6).unwrap();
        let mut indices: Vec<usize> = m.transformer_tap.iter().flatten().copied().collect();
        indices.extend(
            m.transformer_current
                .iter()
                .flatten()
                .flat_map(|pair| pair.iter())
                .flat_map(|a| a.terms.keys())
                .copied(),
        );
        indices.extend(
            m.devices
                .iter()
                .zip(&m.prep.devices)
                .filter(|(_, d)| d.kind == powerio_matrix::McOpfDeviceKind::Ibr)
                .flat_map(|(d, _)| d.power.iter().flatten())
                .copied(),
        );
        assert!(!indices.is_empty());
        for index in indices {
            let mut bad = x.to_vec();
            bad[index] += 0.05;
            assert!(
                validate::check(&m, &bad, f, 1e-6).is_err(),
                "{name}: {}",
                m.problem.var_names[index]
            );
        }
    }
}

#[test]
fn scalar_bmopf_pv_availability_bounds_reach_the_optimized_dispatch() {
    for available in [0.0, 50.0] {
        let data = serde_json::json!({
            "bus":{"b":{"terminal_names":["a","n"],"perfectly_grounded_terminals":["n"]}},
            "voltage_source":{"grid":{"bus":"b","terminal_map":["a","n"],"v_magnitude":[230,0],"v_angle":[0,0],"cost":[1]}},
            "load":{"d":{"bus":"b","terminal_map":["a","n"],"configuration":"SINGLE_PHASE","p_nom":[100],"q_nom":[0]}},
            "ibr":{"pv":{"bus":"b","terminal_map":["a","n"],"topology":"SINGLE_PHASE","prime_mover":"PV","s_max":[1000],"p_avail":available,"p_min":0,"p_max":available,"q_min":0,"q_max":0}}
        });
        let source = powerio::Source::from_memory(
            "availability.bmopf.json",
            serde_json::to_vec(&data).unwrap(),
        )
        .unwrap();
        let parsed = powerio::parse_with_options(
            source,
            &powerio::ParseOptions::default()
                .format("bmopf-json")
                .unwrap(),
        )
        .unwrap();
        let powerio::PioValue::MulticonductorNetwork(net) = parsed.into_value() else {
            panic!("network")
        };
        let result = solve(net);
        let inv = result.devices.iter().find(|d| d.identity == "pv").unwrap();
        near(inv.coil_power_va[0][0], available, 1e-5);
        near(
            result.solution.objective(),
            (100.0 - available) / 1000.0,
            1e-7,
        );
    }
}

#[test]
fn opt_in_profiling_preserves_the_solution() {
    let input = instance(network());
    let baseline = solve_mc_ac_opf_instance(input.clone(), &McOpfOptions::default()).unwrap();
    assert!(baseline.profile.is_none());
    let profiled = solve_mc_ac_opf_instance(
        input,
        &McOpfOptions {
            collect_profile: true,
            ..Default::default()
        },
    )
    .unwrap();
    near(
        baseline.solution.objective(),
        profiled.solution.objective(),
        1e-10,
    );
    assert_eq!(baseline.iterations, profiled.iterations);
    let p = profiled.profile.unwrap();
    assert!(p.stages_s["solve"] > 0.0);
    assert!(p.solver_s["jacobian"] > 0.0);
    assert!(p.evaluations["jacobian"] > 0);
}
