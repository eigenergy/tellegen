use super::sensitivity_tests::{check_fd, outputs, solve, solve_options};
use super::*;
use powerio_dist::{Configuration, DistLoadVoltageModel as Law};
use McOpfParameterKind as Kind;
use McOpfSensitivityOutput as Output;

fn parameter(id: &str, coil: usize, reactive: bool) -> McOpfParameter {
    McOpfParameter::new(
        McOpfParameterOwner::Load,
        id,
        Some(coil),
        if reactive {
            Kind::LoadReactivePower
        } else {
            Kind::LoadActivePower
        },
    )
}
fn laws() -> Vec<Law> {
    vec![
        Law::ConstantCurrent { v_nom: vec![200.] },
        Law::ConstantImpedance { v_nom: vec![200.] },
        Law::Exponential {
            v_nom: vec![200.],
            gamma_p: vec![0.7],
            gamma_q: vec![1.4],
        },
        Law::Zip {
            v_nom: vec![200.],
            alpha_z: vec![0.2],
            alpha_i: vec![0.3],
            alpha_p: vec![0.5],
            beta_z: vec![0.6],
            beta_i: vec![0.1],
            beta_p: vec![0.3],
        },
    ]
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-9 + 2e-5 * b.abs(), "{a} != {b}");
}
fn robust_outputs() -> Vec<Output> {
    outputs()
        .into_iter()
        .filter(|o| {
            !matches!(
                o,
                Output::Voltage {
                    part: McOpfVoltagePart::Magnitude | McOpfVoltagePart::Angle,
                    ..
                }
            )
        })
        .collect()
}
fn marginal(r: &McOpfResult, p: &McOpfParameter) -> f64 {
    let m = r
        .duals
        .as_ref()
        .unwrap()
        .marginals
        .iter()
        .find(|m| &m.parameter == p)
        .unwrap();
    assert_eq!(m.status, McOpfMarginalStatus::Conditional);
    m.derivative.unwrap()
}
fn one_bus(law: Law, p: f64, q: f64) -> powerio::MulticonductorNetwork {
    let mut n = tests::network();
    n.lines_mut().clear();
    n.line_codes_mut().clear();
    n.buses_mut().truncate(1);
    let l = &mut n.loads_mut()[0];
    l.bus = "s".into();
    l.voltage_model = law;
    l.p_nom[0] = p;
    l.q_nom[0] = q;
    n.sources_mut()[0].v_angle[0] = 0.3;
    n
}
fn load_output(q: McOpfFlowQuantity) -> Output {
    Output::Device {
        kind: powerio_matrix::McOpfDeviceKind::Load,
        identity: "d".into(),
        axis: McOpfDeviceAxis::Coil { index: 0 },
        quantity: q,
    }
}

#[test]
fn fixed_voltage_analytic_load_marginals_and_currents_include_voltage_dependence() {
    let r: f64 = 230. / 200.;
    for (law, fp, fq) in laws()
        .into_iter()
        .zip([r, r * r, r.powf(0.7), 0.2 * r * r + 0.3 * r + 0.5])
        .zip([r, r * r, r.powf(1.4), 0.6 * r * r + 0.1 * r + 0.3])
        .map(|((l, p), q)| (l, p, q))
    {
        for (p, q) in [(100., 30.), (0., 0.)] {
            let result = solve(one_bus(law.clone(), p, q));
            let imported =
                McOpfResult::from_module_json(&result.to_module_json().unwrap()).unwrap();
            close(
                marginal(&imported, &parameter("d", 0, false)),
                marginal(&result, &parameter("d", 0, false)),
            );
            let view = result.sensitivities().unwrap();
            for (reactive, f) in [(false, fp), (true, fq)] {
                let parameter = parameter("d", 0, reactive);
                let out = [
                    Output::Objective,
                    load_output(McOpfFlowQuantity::ActivePower),
                    load_output(McOpfFlowQuantity::ReactivePower),
                    load_output(McOpfFlowQuantity::CurrentReal),
                    load_output(McOpfFlowQuantity::CurrentImaginary),
                ];
                let derivatives = view
                    .forward(
                        &[McOpfParameterDirection {
                            parameter: parameter.clone(),
                            delta: 1.,
                        }],
                        &out,
                    )
                    .unwrap();
                let cost = if reactive { 0. } else { 0.0002 * f };
                close(derivatives[0].derivative, cost);
                close(marginal(&result, &parameter), cost);
                close(derivatives[1].derivative, if reactive { 0. } else { f });
                close(derivatives[2].derivative, if reactive { f } else { 0. });
                let angle: f64 = 0.3;
                close(
                    derivatives[3].derivative,
                    f / 230. * if reactive { angle.sin() } else { angle.cos() },
                );
                close(
                    derivatives[4].derivative,
                    f / 230. * if reactive { -angle.cos() } else { angle.sin() },
                );
            }
        }
    }
}

#[test]
fn wye_load_laws_match_cost_and_physical_perturbations() {
    for law in laws() {
        let mut n = tests::rich_network();
        n.generators_mut().clear();
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].q_nom[0] = 30.;
        for reactive in [false, true] {
            check_fd(
                n.clone(),
                parameter("d", 0, reactive),
                &robust_outputs(),
                0.1,
                |n, d| {
                    if reactive {
                        n.loads_mut()[0].q_nom[0] += d;
                    } else {
                        n.loads_mut()[0].p_nom[0] += d;
                    }
                },
            );
        }
    }
}

#[test]
fn zero_nominal_powers_preserve_parameter_laws_across_compiler_forms() {
    let cases = vec![
        (Law::ConstantCurrent { v_nom: vec![200.] }, 0., 0.),
        (Law::ConstantImpedance { v_nom: vec![200.] }, 0., 0.),
        (
            Law::Exponential {
                v_nom: vec![200.],
                gamma_p: vec![0.],
                gamma_q: vec![1.7],
            },
            100.,
            0.,
        ),
        (
            Law::Exponential {
                v_nom: vec![200.],
                gamma_p: vec![1.2],
                gamma_q: vec![2.],
            },
            0.,
            30.,
        ),
        (
            Law::Exponential {
                v_nom: vec![200.],
                gamma_p: vec![2.],
                gamma_q: vec![0.5],
            },
            100.,
            0.,
        ),
        (laws().pop().unwrap(), 0., 0.),
    ];
    for (law, p, q) in cases {
        let mut n = tests::network();
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].p_nom[0] = p;
        n.loads_mut()[0].q_nom[0] = q;
        for reactive in [false, true] {
            check_fd(
                n.clone(),
                parameter("d", 0, reactive),
                &robust_outputs(),
                0.05,
                |n, d| {
                    if reactive {
                        n.loads_mut()[0].q_nom[0] += d;
                    } else {
                        n.loads_mut()[0].p_nom[0] += d;
                    }
                },
            );
        }
    }
}

#[test]
fn delta_zip_coefficients_and_nominal_voltages_follow_coils_after_reordering() {
    let mut n = initial_tests::case("components", "capacitor_delta");
    n.generators_mut().clear();
    n.buses_mut().reverse();
    n.loads_mut().reverse();
    for b in n.buses_mut() {
        b.terminals.reverse();
    }
    let index = n
        .loads()
        .iter()
        .position(|l| l.configuration == Configuration::Delta)
        .unwrap();
    let id = n.loads()[index].name.clone();
    n.loads_mut()[index].voltage_model = Law::Zip {
        v_nom: vec![360., 390., 420.],
        alpha_z: vec![0.2, 0.3, 0.4],
        alpha_i: vec![0.3],
        alpha_p: vec![0.5, 0.4, 0.3],
        beta_z: vec![0.6, 0.5, 0.4],
        beta_i: vec![0.1],
        beta_p: vec![0.3, 0.4, 0.5],
    };
    n.loads_mut()[index].q_nom[1] = 0.;
    let r = solve(n.clone());
    McOpfResult::from_module_json(&r.to_module_json().unwrap()).unwrap();
    let dev = r
        .devices
        .iter()
        .find(|d| d.identity == id && d.kind == powerio_matrix::McOpfDeviceKind::Load)
        .unwrap();
    let mut out = vec![Output::Objective];
    for quantity in [
        McOpfFlowQuantity::ActivePower,
        McOpfFlowQuantity::ReactivePower,
        McOpfFlowQuantity::CurrentReal,
        McOpfFlowQuantity::CurrentImaginary,
    ] {
        for k in 0..3 {
            out.push(Output::Device {
                kind: dev.kind,
                identity: id.clone(),
                axis: McOpfDeviceAxis::Coil { index: k },
                quantity,
            });
        }
        for t in &dev.terminals {
            out.push(Output::Device {
                kind: dev.kind,
                identity: id.clone(),
                axis: McOpfDeviceAxis::Terminal {
                    terminal: t.terminal.clone(),
                },
                quantity,
            });
        }
    }
    for coil in 0..3 {
        for reactive in [false, true] {
            check_fd(
                n.clone(),
                parameter(&id, coil, reactive),
                &out,
                0.05,
                |n, d| {
                    if reactive {
                        n.loads_mut()[index].q_nom[coil] += d;
                    } else {
                        n.loads_mut()[index].p_nom[coil] += d;
                    }
                },
            );
        }
    }
}

#[test]
fn empty_zip_and_exponent_defaults_keep_the_canonical_constant_power_limit() {
    for law in [
        Law::Zip {
            v_nom: vec![200.],
            alpha_z: vec![],
            alpha_i: vec![],
            alpha_p: vec![],
            beta_z: vec![],
            beta_i: vec![],
            beta_p: vec![],
        },
        Law::Exponential {
            v_nom: vec![200.],
            gamma_p: vec![],
            gamma_q: vec![],
        },
    ] {
        for (p, q) in [(100., 20.), (0., 0.)] {
            let result = solve(one_bus(law.clone(), p, q));
            close(marginal(&result, &parameter("d", 0, false)), 0.0002);
            let view = result.sensitivities().unwrap();
            let f = view
                .forward(
                    &[McOpfParameterDirection {
                        parameter: parameter("d", 0, true),
                        delta: 1.,
                    }],
                    &[load_output(McOpfFlowQuantity::ReactivePower)],
                )
                .unwrap();
            close(f[0].derivative, 1.);
        }
    }
}

#[test]
fn load_derivatives_are_invariant_to_working_and_objective_scales() {
    for law in laws() {
        let mut n = tests::network();
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].q_nom[0] = 0.;
        let r = solve(n.clone());
        let view = r.sensitivities().unwrap();
        let out = robust_outputs();
        let dir = [
            McOpfParameterDirection {
                parameter: parameter("d", 0, false),
                delta: 2.,
            },
            McOpfParameterDirection {
                parameter: parameter("d", 0, true),
                delta: -3.,
            },
        ];
        let base = view.forward(&dir, &out).unwrap();
        n.buses_mut().reverse();
        for (vb, sb, scale) in [(100., 4000., 0.2), (400., 700., 7.)] {
            let r = solve_options(
                n.clone(),
                McOpfOptions {
                    collect_duals: true,
                    voltage_base_v: vb,
                    power_base_va: sb,
                    objective_scale: scale,
                    ..Default::default()
                },
            );
            let f = r.sensitivities().unwrap().forward(&dir, &out).unwrap();
            for (a, b) in f.iter().zip(&base) {
                close(a.derivative, b.derivative);
            }
        }
    }
}

#[test]
fn zero_voltage_load_derivative_domains_do_not_change_physical_acceptance() {
    for impedance in [true, false] {
        let law = if impedance {
            Law::ConstantImpedance { v_nom: vec![230.] }
        } else {
            Law::ConstantCurrent { v_nom: vec![230.] }
        };
        let mut n = tests::network();
        n.sources_mut()[0].v_magnitude[0] = 0.;
        n.buses_mut()[1].vpn_min = None;
        n.buses_mut()[1].vpn_max = None;
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].p_nom[0] = 0.;
        let r = solve(n);
        assert!(r.residuals.maximum() < 1e-6);
        let m = r
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == parameter("d", 0, false))
            .unwrap();
        if impedance {
            assert_eq!(m.status, McOpfMarginalStatus::Conditional);
            close(m.derivative.unwrap(), 0.);
        } else {
            assert_eq!(m.status, McOpfMarginalStatus::UnsupportedParameter);
            assert!(m.derivative.is_none());
        }
    }
}

#[test]
fn session_zero_transitions_rebuild_canonical_load_parameter_maps() {
    for law in laws() {
        let mut n = tests::network();
        n.loads_mut()[0].voltage_model = law;
        let mut s = McOpfSession::new(
            McAcOpfInstance::from_network(n).unwrap(),
            McOpfOptions {
                collect_duals: true,
                ..Default::default()
            },
        )
        .unwrap();
        for (p, q) in [(100., 30.), (0., 0.), (0., 30.), (100., 0.)] {
            s.apply_edits(&[McOpfEdit::LoadPower {
                load: "d".into(),
                p_w: vec![p],
                q_var: vec![q],
            }])
            .unwrap();
            assert_eq!(
                s.sensitivities().unwrap_err().kind,
                McOpfSensitivityErrorKind::StaleResult
            );
            s.solve().unwrap();
            let fresh = solve(s.instance().network().clone());
            let params = [parameter("d", 0, false), parameter("d", 0, true)];
            let weights = [McOpfOutputWeight {
                output: Output::Objective,
                weight: 1.,
            }];
            let a = s
                .sensitivities()
                .unwrap()
                .adjoint(&weights, &params)
                .unwrap();
            let b = fresh
                .sensitivities()
                .unwrap()
                .adjoint(&weights, &params)
                .unwrap();
            for (a, b) in a.iter().zip(b) {
                close(a.derivative, b.derivative);
                close(a.derivative, marginal(&fresh, &a.parameter));
            }
        }
    }
}

#[test]
fn current_roundtrip_and_verified_version_two_upgrade_recover_load_derivatives() {
    use serde_json::json;
    let r = solve(one_bus(laws().remove(0), 0., 0.));
    let saved = r.to_module_json().unwrap();
    let read = McOpfResult::from_module_json(&saved).unwrap();
    close(
        marginal(&read, &parameter("d", 0, false)),
        0.0002 * 230. / 200.,
    );
    let prep = initial::prepare(
        r.solution.instance(),
        &McAcOpfAssemblyOptions::new(r.options.voltage_base_v, r.options.power_base_va),
    )
    .unwrap();
    let legacy = model::compile(prep).unwrap();
    let d = r.duals.as_ref().unwrap();
    let lambda: Vec<_> = d.constraints.iter().map(|r| r.multiplier).collect();
    let old = duals::analyze_legacy(
        &legacy,
        &d.primal,
        &lambda,
        &d.lower_bound_multipliers,
        &d.upper_bound_multipliers,
    )
    .unwrap();
    assert!(old
        .marginals
        .iter()
        .filter(|m| m.parameter.owner == McOpfParameterOwner::Load)
        .all(|m| m.status == McOpfMarginalStatus::UnsupportedParameter));
    let mut module = crate::ir::deserialize_module(&saved).unwrap();
    let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
    assert_eq!(report["version"], json!(9));
    report["version"] = json!(2);
    report["duals"] = serde_json::to_value(old).unwrap();
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    let read =
        McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
    close(
        marginal(&read, &parameter("d", 0, false)),
        marginal(&r, &parameter("d", 0, false)),
    );
    assert!(read.sensitivities().is_ok());
    let upgraded = crate::ir::deserialize_module(&read.to_module_json().unwrap()).unwrap();
    assert_eq!(
        upgraded.extensions()[MC_OPF_RESULT_EXTENSION]["version"],
        json!(9)
    );
    report["version"] = json!(3);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
    report["version"] = json!(2);
    report["duals"]["constraints"][0]["multiplier"] = json!(987.);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report)
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
}

#[test]
fn interior_dispatch_sensitivities_include_load_law_mixed_derivatives() {
    for law in laws()
        .into_iter()
        .filter(|l| !matches!(l, Law::ConstantImpedance { .. }))
    {
        let mut n = tests::rich_network();
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].q_nom[0] = 30.;
        n.line_codes_mut()[0].r_series = vec![vec![5., 0.02], vec![0.02, 5.]];
        n.sources_mut()[0].energy_cost_rate = Some(vec![10.]);
        let g = &mut n.generators_mut()[0];
        g.cost = Some(vec![10.]);
        g.p_max = Some(vec![150.]);
        g.s_max = Some(vec![200.]);
        let out = [
            Output::Objective,
            Output::Device {
                kind: powerio_matrix::McOpfDeviceKind::Generator,
                identity: "g".into(),
                axis: McOpfDeviceAxis::Coil { index: 0 },
                quantity: McOpfFlowQuantity::ActivePower,
            },
            Output::Voltage {
                terminal: McOpfTerminalId {
                    bus: "b".into(),
                    terminal: "a".into(),
                },
                part: McOpfVoltagePart::Real,
            },
        ];
        let r = solve(n.clone());
        let p = r
            .devices
            .iter()
            .find(|d| d.identity == "g")
            .unwrap()
            .coil_power_va[0][0];
        assert!(p > 1. && p < 149., "generator must remain interior: {p}");
        for reactive in [false, true] {
            check_fd(n.clone(), parameter("d", 0, reactive), &out, 0.1, |n, d| {
                if reactive {
                    n.loads_mut()[0].q_nom[0] += d;
                } else {
                    n.loads_mut()[0].p_nom[0] += d;
                }
            });
        }
    }
}

pub(super) fn graph(model: &model::Model, x: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
    use pounce_rs::SparsityRequest;
    let mut t = NlTnlp::try_new(model.problem.clone()).unwrap();
    let info = t.get_nlp_info().unwrap();
    let mut g = vec![0.; model.problem.m];
    assert!(t.eval_g(x, true, &mut g));
    // Constant-law parameters live in equality bounds, while general laws
    // embed them in the expression. Compare the complete g - bound residual.
    for (r, value) in g.iter_mut().enumerate() {
        if model.problem.g_l[r] == model.problem.g_u[r] {
            *value -= model.problem.g_l[r];
        }
    }
    let mut rows = vec![0; info.nnz_jac_g as usize];
    let mut cols = rows.clone();
    let mut values = vec![0.; rows.len()];
    assert!(t.eval_jac_g(
        None,
        false,
        SparsityRequest::Structure {
            irow: &mut rows,
            jcol: &mut cols
        }
    ));
    assert!(t.eval_jac_g(
        Some(x),
        true,
        SparsityRequest::Values {
            values: &mut values
        }
    ));
    let mut jac = vec![vec![0.; model.problem.n]; model.problem.m];
    for ((r, c), v) in rows.into_iter().zip(cols).zip(values) {
        jac[r as usize][c as usize] += v;
    }
    (g, jac)
}
fn compiled(n: powerio::MulticonductorNetwork) -> model::Model {
    let instance = McAcOpfInstance::from_network(n).unwrap();
    let prep = initial::prepare(&instance, &Default::default()).unwrap();
    model::compile_with_load_parameters(prep, instance.network()).unwrap()
}

#[test]
fn load_parameter_partials_and_mixed_partials_match_compiled_graph_differences() {
    let mut profiles = laws();
    profiles.push(Law::Exponential {
        v_nom: vec![200.],
        gamma_p: vec![0.],
        gamma_q: vec![0.],
    });
    for law in profiles {
        let mut n = tests::network();
        n.loads_mut()[0].voltage_model = law;
        n.loads_mut()[0].q_nom[0] = 30.;
        let m = compiled(n.clone());
        let x: Vec<_> = m
            .problem
            .x0
            .iter()
            .enumerate()
            .map(|(k, v)| v + 0.03 * ((k + 1) as f64).sin())
            .collect();
        let legacy = model::compile(m.prep.clone()).unwrap();
        assert_eq!(m.problem.var_names, legacy.problem.var_names);
        assert_eq!(m.problem.con_names, legacy.problem.con_names);
        assert_eq!(m.problem.g_l, legacy.problem.g_l);
        assert_eq!(m.problem.g_u, legacy.problem.g_u);
        assert_eq!(m.problem.x0, legacy.problem.x0);
        assert_eq!(graph(&m, &x), graph(&legacy, &x));
        for reactive in [false, true] {
            let mut plus = n.clone();
            let mut minus = n.clone();
            let step = 0.25;
            if reactive {
                plus.loads_mut()[0].q_nom[0] += step;
                minus.loads_mut()[0].q_nom[0] -= step;
            } else {
                plus.loads_mut()[0].p_nom[0] += step;
                minus.loads_mut()[0].p_nom[0] -= step;
            }
            let (gp, jp) = graph(&compiled(plus), &x);
            let (gm, jm) = graph(&compiled(minus), &x);
            for (origin, rule) in &m.dual_map.parameters[&parameter("d", 0, reactive)] {
                let row = m
                    .dual_map
                    .rows
                    .iter()
                    .position(|r| Some(*r) == *origin)
                    .unwrap();
                let duals::Rule::Load(rule) = rule else {
                    panic!("canonical load rule expected")
                };
                let (value, mixed) = rule.evaluate(&x).unwrap();
                close(value, (gp[row] - gm[row]) / (2. * step));
                let mixed: BTreeMap<_, _> = mixed.into_iter().collect();
                for c in 0..m.problem.n {
                    close(
                        mixed.get(&c).copied().unwrap_or(0.),
                        (jp[row][c] - jm[row][c]) / (2. * step),
                    );
                }
            }
        }
    }
}
