use super::initial_tests::case;
use super::sensitivity_tests::{check_fd, physical, solve, solve_options};
use super::*;
use powerio::MulticonductorNetwork;
use serde_json::json;
use McOpfParameterKind as Kind;
use McOpfSensitivityOutput as Output;
use McOpfTransformerAxis as Axis;

fn parameter(id: &str, axis: Option<usize>, kind: Kind) -> McOpfParameter {
    McOpfParameter::new(McOpfParameterOwner::Transformer, id, axis, kind)
}
fn outputs(r: &McOpfResult) -> Vec<Output> {
    let mut outputs = vec![Output::Objective];
    for t in &r.transformers {
        let mut terminals = Vec::new();
        let mut axes: Vec<_> = t
            .coils
            .iter()
            .map(|c| Axis::Coil { index: c.index })
            .collect();
        for c in &t.coils {
            for terminal in std::iter::once(&c.positive).chain(c.negative.as_ref()) {
                if !terminals.contains(terminal) {
                    terminals.push(terminal.clone());
                }
            }
        }
        axes.extend(
            terminals
                .into_iter()
                .map(|terminal| Axis::Terminal { terminal }),
        );
        axes.extend(t.ports.iter().map(|p| Axis::Port {
            name: p.name.clone(),
        }));
        for axis in axes {
            for quantity in [
                McOpfFlowQuantity::ActivePower,
                McOpfFlowQuantity::ReactivePower,
                McOpfFlowQuantity::CurrentReal,
                McOpfFlowQuantity::CurrentImaginary,
            ] {
                outputs.push(Output::Transformer {
                    identity: t.identity.clone(),
                    axis: axis.clone(),
                    quantity,
                });
            }
        }
        if t.tap.is_some() {
            outputs.push(Output::TransformerTap {
                identity: t.identity.clone(),
            });
        }
    }
    outputs
}
fn marginal(r: &McOpfResult, p: &McOpfParameter) -> f64 {
    let m = r
        .duals
        .as_ref()
        .unwrap()
        .marginals
        .iter()
        .find(|m| m.parameter == *p)
        .unwrap();
    assert_eq!(m.status, McOpfMarginalStatus::Conditional, "{m:?}");
    m.derivative.unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8 + 3e-4 * b.abs(), "{a} != {b}");
}
fn port(r: &McOpfResult, name: &str) -> usize {
    r.transformers[0]
        .ports
        .iter()
        .position(|p| p.name == name)
        .unwrap()
}
fn generator(n: &mut MulticonductorNetwork) {
    let mut g = tests::rich_network().generators()[0].clone();
    g.p_max = Some(vec![200.]);
    g.s_max = Some(vec![250.]);
    g.i_max = Some(vec![5., 6.]);
    g.cost = Some(vec![0.4]);
    n.generators_mut().push(g);
}
fn set_number(n: &mut MulticonductorNetwork, key: &str, value: f64) {
    n.transformers_mut()[0]
        .extras
        .insert(key.into(), json!(value));
}
fn increment(n: &mut MulticonductorNetwork, key: &str, d: f64) {
    let t = &mut n.transformers_mut()[0];
    let value = t.extras[key].as_f64().unwrap() + d;
    t.extras.insert(key.into(), json!(value));
}
fn check_direction(
    n: MulticonductorNetwork,
    dir: &[McOpfParameterDirection],
    out: &[Output],
    step: f64,
    edit: impl Fn(&mut MulticonductorNetwork, f64),
) {
    let r = solve(n.clone());
    let view = r.sensitivities().unwrap();
    let f = view.forward(dir, out).unwrap();
    let mut plus = n.clone();
    let mut minus = n;
    edit(&mut plus, step);
    edit(&mut minus, -step);
    let plus = solve(plus);
    let minus = solve(minus);
    for (o, v) in out.iter().zip(f) {
        let fd = (physical(&plus, o) - physical(&minus, o)) / (2. * step);
        assert!(
            (v.derivative - fd).abs() < 2e-8 + 3e-4 * fd.abs(),
            "{o:?}: {} != {fd}",
            v.derivative
        );
        let parameters: Vec<_> = dir.iter().map(|d| d.parameter.clone()).collect();
        let a = view
            .adjoint(
                &[McOpfOutputWeight {
                    output: o.clone(),
                    weight: 1.,
                }],
                &parameters,
            )
            .unwrap();
        close(
            v.derivative,
            a.iter().zip(dir).map(|(a, d)| a.derivative * d.delta).sum(),
        );
        if *o == Output::Objective {
            close(
                v.derivative,
                dir.iter()
                    .map(|d| {
                        r.duals
                            .as_ref()
                            .unwrap()
                            .marginals
                            .iter()
                            .find(|m| m.parameter == d.parameter)
                            .unwrap()
                            .derivative
                            .unwrap()
                            * d.delta
                    })
                    .sum(),
            );
        }
    }
}

#[test]
fn transformer_outputs_follow_winding_terminal_and_rating_port_incidence() {
    for name in [
        "transformer_single_phase",
        "transformer_center_tap",
        "transformer_delta_wye",
        "transformer_wye_delta",
        "transformer_n_winding",
    ] {
        let mut n = case("components", name);
        let count = n.sources()[0].energy_cost_rate.as_ref().unwrap().len();
        n.sources_mut()[0].energy_cost_rate = Some(vec![20.; count]);
        if name == "transformer_wye_delta" {
            // Add an earth reference so tiny zero-sequence
            // solver errors do not dominate the finite-difference flow check.
            n.shunts_mut().push(powerio_dist::DistShunt::new(
                "earth_reference",
                "b",
                vec!["a".into(), "b".into(), "c".into()],
                vec![
                    vec![0.005, 0., 0.],
                    vec![0., 0.005, 0.],
                    vec![0., 0., 0.005],
                ],
                vec![vec![0.; 3]; 3],
            ));
        }
        let r = solve(n.clone());
        let dir: Vec<_> = (0..n.loads()[0].p_nom.len())
            .map(|k| McOpfParameterDirection {
                parameter: McOpfParameter::new(
                    McOpfParameterOwner::Load,
                    &n.loads()[0].name,
                    Some(k),
                    Kind::LoadActivePower,
                ),
                delta: 1.,
            })
            .collect();
        check_direction(n.clone(), &dir, &outputs(&r), 1., |n, d| {
            for p in &mut n.loads_mut()[0].p_nom {
                *p += d;
            }
        });
    }
}

#[test]
fn physical_tap_bound_derivatives_handle_direct_and_inverse_conventions() {
    for name in [
        "tap_single_phase",
        "tap_center_tap",
        "tap_regulator_a",
        "tap_regulator_b",
    ] {
        let mut n = case("controls", name);
        n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
        set_number(&mut n, "g_no_load", 1e-8);
        let r = solve(n.clone());
        let id = &r.transformers[0].identity;
        let prefix = if name.contains("regulator") {
            "tap_ratio"
        } else {
            "tap"
        };
        for (kind, suffix) in [(Kind::TapLower, "min"), (Kind::TapUpper, "max")] {
            let p = parameter(id, None, kind);
            let key = format!("{prefix}_{suffix}");
            check_fd(n.clone(), p.clone(), &outputs(&r), 0.0001, |n, d| {
                increment(n, &key, d)
            });
            let m = marginal(&r, &p);
            assert!(if kind == Kind::TapLower {
                m >= -1e-8
            } else {
                m <= 1e-8
            });
            let f = r
                .sensitivities()
                .unwrap()
                .forward(
                    &[McOpfParameterDirection {
                        parameter: p,
                        delta: 1.,
                    }],
                    &[Output::TransformerTap {
                        identity: id.clone(),
                    }],
                )
                .unwrap();
            let bound = n.transformers()[0].extras[&key].as_f64().unwrap();
            let active = (r.transformers[0].tap.unwrap() - bound).abs() < 1e-5;
            close(f[0].derivative, f64::from(active));
            assert_eq!(f[0].unit, McOpfOutputUnit::Ratio);
        }
    }
}

#[test]
fn interior_tap_responds_to_load_without_active_tap_bounds() {
    let mut n = case("controls", "tap_single_phase");
    set_number(&mut n, "g_no_load", 6e-6);
    n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
    let r = solve(n.clone());
    let tap = r.transformers[0].tap.unwrap();
    assert!(tap > 0.971 && tap < 1.029, "interior tap required: {tap}");
    let p = McOpfParameter::new(
        McOpfParameterOwner::Load,
        &n.loads()[0].name,
        Some(0),
        Kind::LoadActivePower,
    );
    check_fd(n.clone(), p, &outputs(&r), 0.1, |n, d| {
        n.loads_mut()[0].p_nom[0] += d
    });
    for kind in [Kind::TapLower, Kind::TapUpper] {
        assert!(marginal(&r, &parameter(&r.transformers[0].identity, None, kind)).abs() < 1e-8);
    }
}

#[test]
fn binding_transformer_current_ratings_include_exciting_current_ports() {
    for (key, name, cap) in [("i_max_from", "w0:0", 0.3), ("i_max_to", "w1:0", 0.6)] {
        let mut n = case("components", "transformer_single_phase");
        generator(&mut n);
        n.transformers_mut()[0]
            .extras
            .insert(key.into(), json!([cap]));
        let r = solve(n.clone());
        let p = parameter(
            &r.transformers[0].identity,
            Some(port(&r, name)),
            Kind::CurrentRating,
        );
        assert!(marginal(&r, &p) < 0.);
        check_fd(n, p, &outputs(&r), 0.0001, |n, d| {
            let v = n.transformers()[0].extras[key][0].as_f64().unwrap() + d;
            n.transformers_mut()[0]
                .extras
                .insert(key.into(), json!([v]));
        });
    }
}

#[test]
fn binding_apparent_rating_is_independent_of_winding_impedance_base() {
    let mut n = case("components", "transformer_n_winding");
    generator(&mut n);
    n.transformers_mut()[0]
        .extras
        .get_mut("bmopf_winding_metadata")
        .unwrap()["1"]["s_max"] = json!(70.);
    let r = solve(n.clone());
    let p = parameter(
        &r.transformers[0].identity,
        Some(port(&r, "w1:0")),
        Kind::ApparentPowerRating,
    );
    assert!(marginal(&r, &p) < 0.);
    check_fd(n, p, &outputs(&r), 0.1, |n, d| {
        let v = &mut n.transformers_mut()[0]
            .extras
            .get_mut("bmopf_winding_metadata")
            .unwrap()["1"]["s_max"];
        *v = json!(v.as_f64().unwrap() + d);
    });
}

#[test]
fn inactive_and_deselected_transformer_ratings_are_distinguished() {
    let n = case("components", "transformer_n_winding");
    let r = solve(n.clone());
    let p = parameter("tx", Some(port(&r, "w1:0")), Kind::ApparentPowerRating);
    assert!(marginal(&r, &p).abs() < 1e-8);
    check_fd(n.clone(), p.clone(), &outputs(&r), 1., |n, d| {
        let v = &mut n.transformers_mut()[0]
            .extras
            .get_mut("bmopf_winding_metadata")
            .unwrap()["1"]["s_max"];
        *v = json!(v.as_f64().unwrap() + d);
    });
    let i = McAcOpfInstance::from_network(n).unwrap();
    let mut c = i.constraints().clone();
    c.conductor_limits = powerio_prob::ConstraintSelection::None;
    let unselected = solve_mc_ac_opf_instance(
        Arc::new(i.with_constraints(c)),
        &McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    let view = unselected.sensitivities().unwrap();
    assert_eq!(
        view.forward(
            &[McOpfParameterDirection {
                parameter: p,
                delta: 1.
            }],
            &[Output::Objective]
        )
        .unwrap_err()
        .kind,
        McOpfSensitivityErrorKind::UnsupportedParameter
    );
    assert!(view.forward(&[], &outputs(&r)).is_ok());
}

#[test]
fn center_tap_neutral_rating_uses_return_current_measurement() {
    let mut n = case("components", "transformer_center_tap");
    generator(&mut n);
    n.transformers_mut()[0]
        .extras
        .insert("i_max_to".into(), json!([5., 0.15, 5.]));
    let r = solve(n.clone());
    let p = parameter(
        &r.transformers[0].identity,
        Some(port(&r, "w1:neutral")),
        Kind::CurrentRating,
    );
    assert!(marginal(&r, &p) < 0.);
    check_fd(n, p, &outputs(&r), 0.0001, |n, d| {
        let v = &mut n.transformers_mut()[0].extras.get_mut("i_max_to").unwrap()[1];
        *v = json!(v.as_f64().unwrap() + d);
    });
}

#[test]
fn voltage_dependent_loads_exercise_the_opposite_physical_tap_bound() {
    for name in ["tap_single_phase", "tap_regulator_a", "tap_regulator_b"] {
        let mut n = case("controls", name);
        n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
        for l in n.loads_mut() {
            l.voltage_model = powerio_dist::DistLoadVoltageModel::ConstantImpedance {
                v_nom: vec![230.; l.p_nom.len()],
            };
        }
        let r = solve(n.clone());
        let prefix = if name.contains("regulator") {
            "tap_ratio"
        } else {
            "tap"
        };
        let physical = r.transformers[0].tap.unwrap();
        let lo = n.transformers()[0].extras[&format!("{prefix}_min")]
            .as_f64()
            .unwrap();
        let (kind, suffix) = if (physical - lo).abs() < 1e-5 {
            (Kind::TapLower, "min")
        } else {
            (Kind::TapUpper, "max")
        };
        let p = parameter(&r.transformers[0].identity, None, kind);
        assert!(marginal(&r, &p).abs() > 1e-6);
        check_fd(n, p, &outputs(&r), 0.0001, |n, d| {
            increment(n, &format!("{prefix}_{suffix}"), d)
        });
    }
}

#[test]
fn open_delta_regulator_legs_keep_distinct_tap_identities() {
    for name in [
        "tap_open_delta_abbc",
        "tap_open_delta_bcac",
        "tap_open_delta_caba",
    ] {
        let mut n = case("controls", name);
        n.sources_mut()[0].energy_cost_rate = Some(vec![200.; 3]);
        for l in n.loads_mut() {
            l.voltage_model = powerio_dist::DistLoadVoltageModel::ConstantImpedance {
                v_nom: vec![230.; l.p_nom.len()],
            };
        }
        let r = solve(n.clone());
        assert!(r.transformers.len() >= 2);
        let id = n.transformers()[0].name.clone();
        let leg = n.transformers()[0].extras["bmopf_open_delta"]["leg"]
            .as_u64()
            .unwrap() as usize;
        for (kind, key) in [
            (Kind::TapLower, "tap_ratio_min"),
            (Kind::TapUpper, "tap_ratio_max"),
        ] {
            check_fd(
                n.clone(),
                parameter(&id, None, kind),
                &outputs(&r),
                0.0001,
                |n, d| {
                    let v = &mut n.transformers_mut()[0].extras.get_mut(key).unwrap()[leg];
                    *v = json!(v.as_f64().unwrap() + d);
                },
            );
        }
    }
}

#[test]
fn transformer_derivatives_preserve_physical_units_under_scaling_and_order() {
    for rating in [false, true] {
        let mut n = case(
            if rating { "components" } else { "controls" },
            if rating {
                "transformer_single_phase"
            } else {
                "tap_regulator_b"
            },
        );
        if rating {
            generator(&mut n);
            n.transformers_mut()[0]
                .extras
                .insert("i_max_to".into(), json!([0.6]));
        } else {
            n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
            set_number(&mut n, "g_no_load", 1e-8);
        }
        let r = solve(n.clone());
        let out = outputs(&r);
        let p = parameter(
            &r.transformers[0].identity,
            if rating { Some(port(&r, "w1:0")) } else { None },
            if rating {
                Kind::CurrentRating
            } else {
                Kind::TapUpper
            },
        );
        let direction = [McOpfParameterDirection {
            parameter: p.clone(),
            delta: 1.,
        }];
        let base = r
            .sensitivities()
            .unwrap()
            .forward(&direction, &out)
            .unwrap();
        n.buses_mut().reverse();
        for (vb, sb, scale) in [(100., 4000., 2.), (400., 700., 7.)] {
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
            let f = r
                .sensitivities()
                .unwrap()
                .forward(&direction, &out)
                .unwrap();
            for (a, b) in f.iter().zip(&base) {
                close(a.derivative, b.derivative);
            }
            close(marginal(&r, &p), base[0].derivative);
        }
    }
}

#[test]
fn transformer_sessions_refresh_derivatives_and_reject_invalid_output_axes() {
    let mut n = case("controls", "tap_regulator_b");
    n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
    set_number(&mut n, "g_no_load", 1e-8);
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(n).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    s.solve().unwrap();
    let old = s.current_result().unwrap().result.clone();
    let p = McOpfParameter::new(
        McOpfParameterOwner::Load,
        "d",
        Some(0),
        Kind::LoadActivePower,
    );
    let out = outputs(&old);
    let f = old
        .sensitivities()
        .unwrap()
        .forward(
            &[McOpfParameterDirection {
                parameter: p,
                delta: 1.,
            }],
            &out,
        )
        .unwrap();
    s.apply_edits(&[McOpfEdit::LoadPower {
        load: "d".into(),
        p_w: vec![100.1],
        q_var: vec![20.],
    }])
    .unwrap();
    assert!(s.sensitivities().is_err());
    s.solve().unwrap();
    let plus = s.current_result().unwrap().result.clone();
    s.apply_edits(&[McOpfEdit::LoadPower {
        load: "d".into(),
        p_w: vec![99.9],
        q_var: vec![20.],
    }])
    .unwrap();
    s.solve().unwrap();
    let r = &s.current_result().unwrap().result;
    for (o, v) in out.iter().zip(f) {
        close(v.derivative, (physical(&plus, o) - physical(r, o)) / 0.2);
    }
    let saved = McOpfResult::from_module_json(&r.to_module_json().unwrap()).unwrap();
    let view = saved.sensitivities().unwrap();
    for output in [
        Output::TransformerTap {
            identity: "missing".into(),
        },
        Output::Transformer {
            identity: "tx".into(),
            axis: Axis::Coil { index: usize::MAX },
            quantity: McOpfFlowQuantity::ActivePower,
        },
        Output::Transformer {
            identity: "tx".into(),
            axis: Axis::Port {
                name: "missing".into(),
            },
            quantity: McOpfFlowQuantity::CurrentReal,
        },
        Output::Transformer {
            identity: "tx".into(),
            axis: Axis::Terminal {
                terminal: McOpfTerminalId {
                    bus: "missing".into(),
                    terminal: "a".into(),
                },
            },
            quantity: McOpfFlowQuantity::ReactivePower,
        },
    ] {
        let e = view.forward(&[], &[output]).unwrap_err();
        assert_eq!(e.kind, McOpfSensitivityErrorKind::UnsupportedOutput);
    }
    assert!(old.sensitivities().is_ok());
}

#[test]
fn current_reports_verify_old_maps_before_upgrading_transformer_claims() {
    let mut n = case("controls", "tap_regulator_b");
    n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
    set_number(&mut n, "g_no_load", 1e-8);
    let r = solve(n);
    let prep = initial::prepare(
        r.solution.instance(),
        &McAcOpfAssemblyOptions::new(r.options.voltage_base_v, r.options.power_base_va),
    )
    .unwrap();
    let legacy =
        model::compile_with_inverter_parameters(prep.clone(), r.solution.instance().network())
            .unwrap();
    let current = model::compile_with_parameters(prep, r.solution.instance().network()).unwrap();
    assert_eq!(legacy.problem.var_names, current.problem.var_names);
    assert_eq!(legacy.problem.con_names, current.problem.con_names);
    assert_eq!(legacy.problem.g_l, current.problem.g_l);
    assert_eq!(legacy.problem.g_u, current.problem.g_u);
    assert_eq!(legacy.problem.x0, current.problem.x0);
    let d = r.duals.as_ref().unwrap();
    assert_eq!(
        load_parameter_tests::graph(&legacy, &d.primal),
        load_parameter_tests::graph(&current, &d.primal)
    );
    let lambda: Vec<_> = d.constraints.iter().map(|c| c.multiplier).collect();
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
        .all(|m| m.parameter.owner != McOpfParameterOwner::Transformer));
    let saved = r.to_module_json().unwrap();
    let mut module = crate::ir::deserialize_module(&saved).unwrap();
    let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
    assert_eq!(report["version"], json!(8));
    report["version"] = json!(4);
    report["duals"] = serde_json::to_value(old).unwrap();
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    let read =
        McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
    let p = parameter("tx", None, Kind::TapUpper);
    close(marginal(&read, &p), marginal(&r, &p));
    assert!(read.sensitivities().is_ok());
    assert_eq!(
        crate::ir::deserialize_module(&read.to_module_json().unwrap())
            .unwrap()
            .extensions()[MC_OPF_RESULT_EXTENSION]["version"],
        json!(8)
    );
    report["version"] = json!(5);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
    report["version"] = json!(4);
    report["duals"]["constraints"][0]["multiplier"] = json!(987.);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report)
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
}

#[test]
fn fixed_taps_and_zero_ratings_do_not_gain_unsupported_derivatives() {
    let mut n = case("components", "transformer_single_phase");
    let r = solve(n.clone());
    let view = r.sensitivities().unwrap();
    assert_eq!(
        view.forward(
            &[],
            &[Output::TransformerTap {
                identity: "tx".into()
            }]
        )
        .unwrap_err()
        .kind,
        McOpfSensitivityErrorKind::UnsupportedOutput
    );
    assert_eq!(
        view.forward(
            &[McOpfParameterDirection {
                parameter: parameter("tx", None, Kind::TapLower),
                delta: 1.
            }],
            &[Output::Objective]
        )
        .unwrap_err()
        .kind,
        McOpfSensitivityErrorKind::UnsupportedParameter
    );
    for l in n.loads_mut() {
        l.p_nom.fill(0.);
        l.q_nom.fill(0.);
    }
    set_number(&mut n, "g_no_load", 0.);
    set_number(&mut n, "b_no_load", 0.);
    n.sources_mut()[0].energy_cost_rate = Some(vec![0.]);
    let zero = solve(n.clone());
    n.transformers_mut()[0]
        .extras
        .insert("i_max_from".into(), json!([0.]));
    let i = McAcOpfInstance::from_network(n).unwrap();
    let prep = initial::prepare(&i, &Default::default()).unwrap();
    let m = model::compile_with_parameters(prep, i.network()).unwrap();
    let d = duals::analyze(
        &m,
        &zero.duals.as_ref().unwrap().primal,
        &vec![0.; m.problem.m],
        &vec![0.; m.problem.n],
        &vec![0.; m.problem.n],
    )
    .unwrap();
    assert_eq!(d.status, McOpfMarginalStatus::ZeroLimitReduction);
    assert!(d.marginals.iter().all(|m| m.derivative.is_none()));
}
