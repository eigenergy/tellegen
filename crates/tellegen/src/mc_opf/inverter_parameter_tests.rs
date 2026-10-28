use super::sensitivity_tests::{check_fd, physical, solve, solve_options};
use super::*;
use powerio::MulticonductorNetwork;
use powerio_dist::{
    ActivePowerReference as Reference, DistControlProfile, DistIbr, IbrPrimeMover, IbrTopology,
    IbrVoltageAggregation,
};
use serde_json::json;
use McOpfParameterKind as Kind;
use McOpfSensitivityOutput as Output;

fn parameter(axis: usize, kind: Kind) -> McOpfParameter {
    McOpfParameter::new(McOpfParameterOwner::Inverter, "inv", Some(axis), kind)
}
pub(super) fn single() -> MulticonductorNetwork {
    let mut n = tests::network();
    n.loads_mut()[0].q_nom[0] = 20.;
    let mut inv = DistIbr::new(
        "inv",
        "b",
        vec!["a".into(), "n".into()],
        IbrTopology::SinglePhase,
        IbrPrimeMover::Pv,
        vec![80.],
    );
    inv.p_min = Some(vec![0.]);
    inv.p_max = Some(vec![40.]);
    inv.p_avail = Some(40.);
    inv.q_min = Some(vec![0.]);
    inv.q_max = Some(vec![0.]);
    inv.i_max = Some(vec![2., 3.]);
    inv.extras.insert("cost".into(), json!([0.03]));
    n.ibrs_mut().push(inv);
    n
}
pub(super) fn three(delta: bool) -> MulticonductorNetwork {
    let mut n = initial_tests::case(
        "components",
        if delta {
            "ibr_three_leg"
        } else {
            "ibr_four_leg"
        },
    );
    n.generators_mut().clear();
    n.ibrs_mut()[0].p_avail = Some(120.);
    n.ibrs_mut()[0].q_min = Some(vec![0.; 3]);
    n.ibrs_mut()[0].q_max = Some(vec![0.; 3]);
    n
}
pub(super) fn outputs(n: &MulticonductorNetwork) -> Vec<Output> {
    let mut out = vec![Output::Objective];
    for k in 0..n.ibrs()[0].s_max.len() {
        for quantity in [
            McOpfFlowQuantity::ActivePower,
            McOpfFlowQuantity::ReactivePower,
            McOpfFlowQuantity::CurrentReal,
            McOpfFlowQuantity::CurrentImaginary,
        ] {
            out.push(Output::Device {
                kind: powerio_matrix::McOpfDeviceKind::Ibr,
                identity: "inv".into(),
                axis: McOpfDeviceAxis::Coil { index: k },
                quantity,
            });
        }
    }
    for t in &n.ibrs()[0].terminal_map {
        out.push(Output::Device {
            kind: powerio_matrix::McOpfDeviceKind::Ibr,
            identity: "inv".into(),
            axis: McOpfDeviceAxis::Terminal {
                terminal: t.clone(),
            },
            quantity: McOpfFlowQuantity::CurrentReal,
        });
        out.push(Output::Voltage {
            terminal: McOpfTerminalId {
                bus: "b".into(),
                terminal: t.clone(),
            },
            part: McOpfVoltagePart::Real,
        });
    }
    out
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
fn availability(n: &mut MulticonductorNetwork, k: usize, d: f64) {
    let inv = &mut n.ibrs_mut()[0];
    inv.p_max.as_mut().unwrap()[k] += d;
    inv.p_avail = Some(inv.p_max.as_ref().unwrap().iter().sum());
}
pub(super) fn control(n: &mut MulticonductorNetwork, reference: Reference, average: bool) {
    let mut p = DistControlProfile::new("cp");
    p.volt_watt = Some(
        serde_json::from_value(
            json!({"breakpoints":[220.,240.],"p_limits":[0.1,0.9],"p_ref":reference}),
        )
        .unwrap(),
    );
    p.volt_var = Some(
        serde_json::from_value(json!({"breakpoints":[210.,220.,225.,240.],"q_limits":[-0.2,0.2]}))
            .unwrap(),
    );
    n.control_profiles_mut().push(p);
    let inv = &mut n.ibrs_mut()[0];
    inv.control_profile = Some("cp".into());
    inv.q_min = Some(vec![-30.; inv.s_max.len()]);
    inv.q_max = Some(vec![30.; inv.s_max.len()]);
    inv.voltage_aggregation = Some(if average {
        IbrVoltageAggregation::Average
    } else {
        IbrVoltageAggregation::PerPhase
    });
}

#[test]
fn availability_and_power_capabilities_match_perturbed_dispatch() {
    for n in [single(), three(false), three(true)] {
        let out = outputs(&n);
        for k in 0..n.ibrs()[0].s_max.len() {
            assert!(marginal(&solve(n.clone()), &parameter(k, Kind::Availability)) < 0.);
            check_fd(
                n.clone(),
                parameter(k, Kind::Availability),
                &out,
                0.1,
                |n, d| availability(n, k, d),
            );
            check_fd(
                n.clone(),
                parameter(k, Kind::ActivePowerUpper),
                &out,
                0.1,
                |n, d| n.ibrs_mut()[0].p_max.as_mut().unwrap()[k] += d,
            );
        }
    }
}

#[test]
fn lower_active_and_reactive_capability_sides_match_perturbed_dispatch() {
    for kind in [
        Kind::ActivePowerLower,
        Kind::ReactivePowerLower,
        Kind::ReactivePowerUpper,
    ] {
        let mut n = single();
        if kind != Kind::ActivePowerLower {
            n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
            n.ibrs_mut()[0].extras.insert("cost".into(), json!([3.]));
        }
        match kind {
            Kind::ActivePowerLower => {
                n.ibrs_mut()[0].extras.insert("cost".into(), json!([0.4]));
                n.ibrs_mut()[0].p_min = Some(vec![10.]);
            }
            Kind::ReactivePowerLower => n.ibrs_mut()[0].q_min = Some(vec![40.]),
            Kind::ReactivePowerUpper => n.ibrs_mut()[0].q_max = Some(vec![5.]),
            _ => unreachable!(),
        }
        if kind == Kind::ReactivePowerLower {
            n.ibrs_mut()[0].q_max = Some(vec![60.]);
        }
        check_fd(n.clone(), parameter(0, kind), &outputs(&n), 0.1, |n, d| {
            let i = &mut n.ibrs_mut()[0];
            let v = match kind {
                Kind::ActivePowerLower => &mut i.p_min,
                Kind::ReactivePowerLower => &mut i.q_min,
                _ => &mut i.q_max,
            };
            v.as_mut().unwrap()[0] += d;
        });
    }
}

#[test]
fn binding_current_and_apparent_ratings_include_physical_unit_scaling() {
    for delta in [false, true] {
        for kind in [Kind::CurrentRating, Kind::ApparentPowerRating] {
            let mut n = three(delta);
            let i = &mut n.ibrs_mut()[0];
            if kind == Kind::CurrentRating {
                i.i_max.as_mut().unwrap()[0] = 0.05;
            } else {
                i.s_max[0] = 20.;
            }
            assert!(marginal(&solve(n.clone()), &parameter(0, kind)) < 0.);
            check_fd(
                n.clone(),
                parameter(0, kind),
                &outputs(&n),
                if kind == Kind::CurrentRating {
                    0.0001
                } else {
                    0.05
                },
                |n, d| {
                    let i = &mut n.ibrs_mut()[0];
                    if kind == Kind::CurrentRating {
                        i.i_max.as_mut().unwrap()[0] += d;
                    } else {
                        i.s_max[0] += d;
                    }
                },
            );
        }
    }
}

#[test]
fn single_phase_return_minimum_has_correct_axis_and_refuses_ties() {
    for axis in [0, 1] {
        let mut n = single();
        n.ibrs_mut()[0].i_max.as_mut().unwrap()[axis] = 0.1;
        check_fd(
            n.clone(),
            parameter(axis, Kind::CurrentRating),
            &outputs(&n),
            0.0001,
            |n, d| n.ibrs_mut()[0].i_max.as_mut().unwrap()[axis] += d,
        );
        let other = parameter(1 - axis, Kind::CurrentRating);
        assert_eq!(marginal(&solve(n.clone()), &other), 0.);
        check_fd(n.clone(), other, &outputs(&n), 0.01, |n, d| {
            n.ibrs_mut()[0].i_max.as_mut().unwrap()[1 - axis] += d
        });
    }
    let mut n = single();
    n.ibrs_mut()[0].i_max = Some(vec![0.1, 0.1]);
    let r = solve(n);
    let s = r.sensitivities().unwrap();
    for k in [0, 1] {
        let p = parameter(k, Kind::CurrentRating);
        assert!(s
            .forward(
                &[McOpfParameterDirection {
                    parameter: p,
                    delta: 1.
                }],
                &[Output::Objective]
            )
            .is_err());
    }
}

#[test]
fn four_leg_neutral_rating_derivatives_follow_terminal_map() {
    let mut n = three(false);
    let i = &mut n.ibrs_mut()[0];
    i.p_max = Some(vec![15., 40., 60.]);
    i.p_avail = Some(115.);
    i.i_max.as_mut().unwrap()[3] = 0.03;
    assert!(marginal(&solve(n.clone()), &parameter(3, Kind::CurrentRating)) < 0.);
    check_fd(
        n.clone(),
        parameter(3, Kind::CurrentRating),
        &outputs(&n),
        0.0001,
        |n, d| n.ibrs_mut()[0].i_max.as_mut().unwrap()[3] += d,
    );
    // Moving the neutral in terminal_map moves the current-rating axis, while
    // the per-coil power fields continue to follow the non-neutral order.
    let i = &mut n.ibrs_mut()[0];
    i.terminal_map.rotate_right(1);
    i.i_max.as_mut().unwrap().rotate_right(1);
    check_fd(
        n.clone(),
        parameter(0, Kind::CurrentRating),
        &outputs(&n),
        0.0001,
        |n, d| n.ibrs_mut()[0].i_max.as_mut().unwrap()[0] += d,
    );
}

#[test]
fn control_references_include_cross_coil_availability_and_nameplate_effects() {
    for reference in [Reference::PMax, Reference::PAvailable, Reference::SMax] {
        for average in [false, true] {
            let mut n = three(false);
            control(&mut n, reference, average);
            let out = outputs(&n);
            check_fd(
                n.clone(),
                parameter(0, Kind::Availability),
                &out,
                0.1,
                |n, d| availability(n, 0, d),
            );
            check_fd(
                n.clone(),
                parameter(0, Kind::ActivePowerUpper),
                &out,
                0.1,
                |n, d| n.ibrs_mut()[0].p_max.as_mut().unwrap()[0] += d,
            );
            check_fd(
                n.clone(),
                parameter(0, Kind::ApparentPowerRating),
                &out,
                0.1,
                |n, d| n.ibrs_mut()[0].s_max[0] += d,
            );
            if reference == Reference::PAvailable {
                let r = solve(n);
                let s = r.sensitivities().unwrap();
                let f = s
                    .forward(
                        &[McOpfParameterDirection {
                            parameter: parameter(0, Kind::Availability),
                            delta: 1.,
                        }],
                        &out,
                    )
                    .unwrap();
                // One coil's availability affects the active power of all coils.
                for k in 0..3 {
                    assert!(f[1 + 4 * k].derivative > 0.1);
                }
            }
        }
    }
}

#[test]
fn inactive_availability_and_ratings_have_zero_local_value() {
    let mut n = single();
    n.ibrs_mut()[0].s_max[0] = 20.;
    let p = parameter(0, Kind::Availability);
    assert!(marginal(&solve(n.clone()), &p).abs() < 1e-8);
    check_fd(n.clone(), p, &outputs(&n), 0.1, |n, d| {
        availability(n, 0, d)
    });
    let n = single();
    check_fd(
        n.clone(),
        parameter(0, Kind::ApparentPowerRating),
        &outputs(&n),
        0.1,
        |n, d| n.ibrs_mut()[0].s_max[0] += d,
    );
}

#[test]
fn availability_rejects_incoherent_totals_and_zero_allocation_without_hiding_capabilities() {
    let mut n = single();
    control(&mut n, Reference::PAvailable, false);
    n.ibrs_mut()[0].p_avail = Some(35.);
    let r = solve(n);
    let p = parameter(0, Kind::Availability);
    let m = r
        .duals
        .as_ref()
        .unwrap()
        .marginals
        .iter()
        .find(|m| m.parameter == p)
        .unwrap();
    assert_eq!(m.status, McOpfMarginalStatus::UnsupportedParameter);
    assert!(r
        .sensitivities()
        .unwrap()
        .forward(
            &[McOpfParameterDirection {
                parameter: p,
                delta: 1.
            }],
            &[Output::Objective]
        )
        .is_err());
    assert!(marginal(&r, &parameter(0, Kind::ApparentPowerRating)).is_finite());
    let mut n = single();
    n.ibrs_mut()[0].p_max = Some(vec![0.]);
    n.ibrs_mut()[0].p_avail = Some(0.);
    let r = solve(n.clone());
    assert_eq!(
        r.duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == parameter(0, Kind::Availability))
            .unwrap()
            .status,
        McOpfMarginalStatus::UnsupportedParameter
    );
    n.ibrs_mut()[0].s_max[0] = 0.;
    let r = solve(n);
    assert_eq!(
        r.duals.as_ref().unwrap().status,
        McOpfMarginalStatus::ZeroLimitReduction
    );
    assert!(r.sensitivities().is_err());
}

#[test]
fn control_reference_derivatives_survive_deselected_capability_rows() {
    let mut n = single();
    control(&mut n, Reference::PMax, false);
    let solve_unselected = |n| {
        let i = McAcOpfInstance::from_network(n).unwrap();
        let mut c = i.constraints().clone();
        c.generator_capability = powerio_prob::ConstraintSelection::None;
        solve_mc_ac_opf_instance(
            Arc::new(i.with_constraints(c)),
            &McOpfOptions {
                collect_duals: true,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let r = solve_unselected(n.clone());
    let p = parameter(0, Kind::Availability);
    let f = r
        .sensitivities()
        .unwrap()
        .forward(
            &[McOpfParameterDirection {
                parameter: p.clone(),
                delta: 1.,
            }],
            &outputs(&n),
        )
        .unwrap();
    let mut plus = n.clone();
    let mut minus = n.clone();
    availability(&mut plus, 0, 0.1);
    availability(&mut minus, 0, -0.1);
    let plus = solve_unselected(plus);
    let minus = solve_unselected(minus);
    for (o, v) in outputs(&n).iter().zip(f) {
        close(
            v.derivative,
            (physical(&plus, o) - physical(&minus, o)) / 0.2,
        );
    }
    assert!(marginal(&r, &p) < 0.);
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8 + 3e-4 * b.abs(), "{a} != {b}");
}

#[test]
fn inverter_derivatives_preserve_units_under_bases_objective_scale_and_bus_order() {
    let mut n = single();
    control(&mut n, Reference::PAvailable, false);
    let out = outputs(&n);
    let dir = [
        McOpfParameterDirection {
            parameter: parameter(0, Kind::Availability),
            delta: 2.,
        },
        McOpfParameterDirection {
            parameter: parameter(0, Kind::ApparentPowerRating),
            delta: -3.,
        },
    ];
    let base = solve(n.clone())
        .sensitivities()
        .unwrap()
        .forward(&dir, &out)
        .unwrap();
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

#[test]
fn retained_availability_edits_rebuild_derivatives_and_saved_reports() {
    let mut n = single();
    control(&mut n, Reference::PAvailable, false);
    let mut session = McOpfSession::new(
        McAcOpfInstance::from_network(n).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    session.solve().unwrap();
    let historical = session.current_result().unwrap().result.clone();
    let p = parameter(0, Kind::Availability);
    let d = marginal(&historical, &p);
    session
        .apply_edits(&[McOpfEdit::InverterAvailability {
            inverter: "inv".into(),
            p_max_w: Some(vec![40.1]),
        }])
        .unwrap();
    assert!(session.sensitivities().is_err());
    session.solve().unwrap();
    let r = &session.current_result().unwrap().result;
    close(
        d,
        (r.solution.objective() - historical.solution.objective()) / 0.1,
    );
    let saved = r.to_module_json().unwrap();
    let read = McOpfResult::from_module_json(&saved).unwrap();
    close(marginal(&read, &p), marginal(r, &p));
    assert!(read.sensitivities().is_ok());
    assert!(historical.sensitivities().is_ok());
    let fresh = solve(session.instance().network().clone());
    close(marginal(&fresh, &p), marginal(r, &p));
}

#[test]
fn current_reports_verify_legacy_maps_before_upgrading_inverter_claims() {
    let mut n = single();
    control(&mut n, Reference::PAvailable, false);
    let r = solve(n);
    let saved = r.to_module_json().unwrap();
    let d = r.duals.as_ref().unwrap();
    let prep = initial::prepare(
        r.solution.instance(),
        &McAcOpfAssemblyOptions::new(r.options.voltage_base_v, r.options.power_base_va),
    )
    .unwrap();
    for version in [2, 3] {
        let old_model = if version == 2 {
            model::compile(prep.clone())
        } else {
            model::compile_with_load_parameters(prep.clone(), r.solution.instance().network())
        }
        .unwrap();
        let lambda: Vec<_> = d.constraints.iter().map(|r| r.multiplier).collect();
        let old = duals::analyze_legacy(
            &old_model,
            &d.primal,
            &lambda,
            &d.lower_bound_multipliers,
            &d.upper_bound_multipliers,
        )
        .unwrap();
        assert!(old
            .marginals
            .iter()
            .all(|m| m.parameter.owner != McOpfParameterOwner::Inverter));
        let mut module = crate::ir::deserialize_module(&saved).unwrap();
        let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
        assert_eq!(report["version"], json!(9));
        report["version"] = json!(version);
        report["duals"] = serde_json::to_value(old).unwrap();
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
            .unwrap();
        let read =
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
        close(
            marginal(&read, &parameter(0, Kind::Availability)),
            marginal(&r, &parameter(0, Kind::Availability)),
        );
        assert!(read.sensitivities().is_ok());
        assert_eq!(
            crate::ir::deserialize_module(&read.to_module_json().unwrap())
                .unwrap()
                .extensions()[MC_OPF_RESULT_EXTENSION]["version"],
            json!(9)
        );
        report["version"] = json!(4);
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
            .unwrap();
        assert!(
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err()
        );
        report["version"] = json!(version);
        report["duals"]["constraints"][0]["multiplier"] = json!(987.);
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report)
            .unwrap();
        assert!(
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err()
        );
    }
}

#[test]
fn interior_dispatch_uses_control_reference_mixed_derivatives() {
    let mut n = single();
    control(&mut n, Reference::SMax, false);
    n.control_profiles_mut()[0].volt_watt = None;
    n.sources_mut()[0].energy_cost_rate = Some(vec![10.]);
    n.line_codes_mut()[0].r_series = vec![vec![5., 0.02], vec![0.02, 5.]];
    let i = &mut n.ibrs_mut()[0];
    i.extras.insert("cost".into(), json!([10.]));
    i.s_max[0] = 200.;
    i.p_max = Some(vec![150.]);
    i.p_avail = Some(150.);
    i.q_min = Some(vec![-100.]);
    i.q_max = Some(vec![100.]);
    let r = solve(n.clone());
    let p = r
        .devices
        .iter()
        .find(|d| d.identity == "inv")
        .unwrap()
        .coil_power_va[0][0];
    assert!(p > 1. && p < 149., "dispatch must stay interior: {p}");
    check_fd(
        n.clone(),
        parameter(0, Kind::ApparentPowerRating),
        &outputs(&n),
        0.1,
        |n, d| n.ibrs_mut()[0].s_max[0] += d,
    );
}

#[test]
fn canonical_control_parameter_partials_match_compiled_residuals_and_jacobians() {
    use super::load_parameter_tests::graph;
    let compile = |n| {
        let i = McAcOpfInstance::from_network(n).unwrap();
        let prep = initial::prepare(&i, &Default::default()).unwrap();
        model::compile_with_parameters(prep, i.network()).unwrap()
    };
    for reference in [Reference::PMax, Reference::PAvailable, Reference::SMax] {
        let mut n = three(false);
        control(&mut n, reference, true);
        let m = compile(n.clone());
        let old = model::compile_with_load_parameters(m.prep.clone(), &n).unwrap();
        assert_eq!(m.problem.var_names, old.problem.var_names);
        assert_eq!(m.problem.con_names, old.problem.con_names);
        assert_eq!(m.problem.g_l, old.problem.g_l);
        assert_eq!(m.problem.g_u, old.problem.g_u);
        for voltage in [100., 220., 225., 230., 240., 350.] {
            let mut x = m.problem.x0.clone();
            for (c, name) in m.problem.var_names.iter().enumerate() {
                if name.contains(":magnitude:") {
                    x[c] = voltage / m.prep.bases.voltage_base_v;
                }
            }
            assert_eq!(graph(&m, &x), graph(&old, &x));
            for kind in [
                Kind::Availability,
                Kind::ActivePowerUpper,
                Kind::ApparentPowerRating,
            ] {
                let p = parameter(0, kind);
                let mut plus = n.clone();
                let mut minus = n.clone();
                for (net, d) in [(&mut plus, 0.01), (&mut minus, -0.01)] {
                    match kind {
                        Kind::Availability => availability(net, 0, d),
                        Kind::ActivePowerUpper => net.ibrs_mut()[0].p_max.as_mut().unwrap()[0] += d,
                        _ => net.ibrs_mut()[0].s_max[0] += d,
                    }
                }
                let (gp, jp) = graph(&compile(plus), &x);
                let (gm, jm) = graph(&compile(minus), &x);
                for (origin, rule) in &m.dual_map.parameters[&p] {
                    let duals::Rule::Inverter(control) = rule else {
                        continue;
                    };
                    let r = m
                        .dual_map
                        .rows
                        .iter()
                        .position(|r| Some(*r) == *origin)
                        .unwrap();
                    let (value, gradient) = control.evaluate(&x).unwrap();
                    close(value, (gp[r] - gm[r]) / 0.02);
                    for c in 0..x.len() {
                        let expected = gradient
                            .iter()
                            .filter(|(col, _)| *col == c)
                            .map(|(_, v)| v)
                            .sum::<f64>();
                        close(expected, (jp[r][c] - jm[r][c]) / 0.02);
                    }
                }
            }
        }
    }
}
