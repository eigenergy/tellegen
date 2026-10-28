use super::inverter_parameter_tests::{control, outputs, single, three};
use super::sensitivity_tests::{check_fd, physical, solve, solve_options};
use super::*;
use powerio::MulticonductorNetwork;
use powerio_dist::{ActivePowerReference as Reference, DistControlProfile};
use serde_json::json;
use McOpfParameterKind as Kind;
use McOpfSensitivityOutput as Output;

fn parameter(kind: Kind, axis: Option<usize>) -> McOpfParameter {
    McOpfParameter::new(McOpfParameterOwner::ControlProfile, "cp", axis, kind)
}
fn droop(reference: Reference, average: bool) -> MulticonductorNetwork {
    let mut n = single();
    control(&mut n, reference, average);
    n.control_profiles_mut()[0]
        .volt_var
        .as_mut()
        .unwrap()
        .breakpoints = vec![210., 220., 229., 240.];
    n.sources_mut()[0].energy_cost_rate = Some(vec![20.]);
    n.ibrs_mut()[0].extras.insert("cost".into(), json!([3.]));
    n.ibrs_mut()[0].s_max[0] = 60.;
    n
}
fn parameters() -> Vec<McOpfParameter> {
    [
        (Kind::VoltVarBreakpoint, 4),
        (Kind::VoltWattBreakpoint, 2),
        (Kind::VoltVarFraction, 2),
        (Kind::VoltWattFraction, 2),
    ]
    .into_iter()
    .flat_map(|(kind, n)| (0..n).map(move |k| parameter(kind, Some(k))))
    .collect()
}
fn perturb(n: &mut MulticonductorNetwork, p: &McOpfParameter, d: f64) {
    let c = n
        .control_profiles_mut()
        .iter_mut()
        .find(|c| c.name == p.component)
        .unwrap();
    let values = match p.kind {
        Kind::VoltVarBreakpoint => &mut c.volt_var.as_mut().unwrap().breakpoints,
        Kind::VoltWattBreakpoint => &mut c.volt_watt.as_mut().unwrap().breakpoints,
        Kind::VoltVarFraction => &mut c.volt_var.as_mut().unwrap().q_limits,
        Kind::VoltWattFraction => &mut c.volt_watt.as_mut().unwrap().p_limits,
        Kind::PowerFactor => {
            c.power_factor.as_mut().unwrap().pf += d;
            return;
        }
        _ => unreachable!(),
    };
    values[p.axis.unwrap()] += d;
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8 + 3e-4 * b.abs(), "{a} != {b}");
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

#[test]
fn control_breakpoints_and_fractions_match_cost_forward_and_adjoint_differences() {
    for reference in [Reference::PMax, Reference::PAvailable, Reference::SMax] {
        let n = droop(reference, false);
        for p in parameters() {
            let step = if matches!(p.kind, Kind::VoltVarBreakpoint | Kind::VoltWattBreakpoint) {
                0.002
            } else {
                0.00002
            };
            check_fd(n.clone(), p.clone(), &outputs(&n), step, |n, d| {
                perturb(n, &p, d)
            });
        }
    }
}

#[test]
fn shared_profile_derivatives_sum_all_inverters_and_follow_profile_identity() {
    let mut n = droop(Reference::PAvailable, false);
    let mut other = n.ibrs()[0].clone();
    other.name = "other".into();
    other.bus = "s".into();
    other.s_max[0] = 100.;
    other.p_max = Some(vec![60.]);
    other.p_avail = Some(60.);
    n.ibrs_mut().push(other);
    let mut out = outputs(&n);
    for quantity in [
        McOpfFlowQuantity::ActivePower,
        McOpfFlowQuantity::ReactivePower,
    ] {
        out.push(Output::Device {
            kind: powerio_matrix::McOpfDeviceKind::Ibr,
            identity: "other".into(),
            axis: McOpfDeviceAxis::Coil { index: 0 },
            quantity,
        });
    }
    for p in [
        parameter(Kind::VoltWattFraction, Some(1)),
        parameter(Kind::VoltVarBreakpoint, Some(2)),
    ] {
        let r = solve(n.clone());
        let m = r
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == p)
            .unwrap();
        assert_eq!(m.row_ordinals.len(), 2);
        check_fd(n.clone(), p.clone(), &out, 0.0002, |n, d| perturb(n, &p, d));
    }
    let p = parameter(Kind::VoltWattFraction, Some(1));
    let r = solve(n.clone());
    let dir = [McOpfParameterDirection {
        parameter: p.clone(),
        delta: 1.,
    }];
    let base = r.sensitivities().unwrap().forward(&dir, &out).unwrap();
    n.ibrs_mut().reverse();
    n.buses_mut().reverse();
    n.control_profiles_mut()
        .insert(0, DistControlProfile::new("unused"));
    let r = solve(n);
    let changed = r.sensitivities().unwrap().forward(&dir, &out).unwrap();
    for (a, b) in base.iter().zip(changed) {
        close(a.derivative, b.derivative);
    }
}

#[test]
fn averaged_monitors_and_voltage_reference_families_have_profile_derivatives() {
    for reference in ["PN_PER_PHASE", "PG_PER_PHASE", "PP_PER_PHASE"] {
        let mut n = three(false);
        control(&mut n, Reference::PMax, true);
        let c = &mut n.control_profiles_mut()[0];
        let vref = serde_json::from_value(json!(reference)).unwrap();
        c.volt_var.as_mut().unwrap().voltage_reference = Some(vref);
        c.volt_watt.as_mut().unwrap().voltage_reference = Some(vref);
        if reference == "PP_PER_PHASE" {
            for v in &mut c.volt_var.as_mut().unwrap().breakpoints {
                *v *= 3_f64.sqrt();
            }
            for v in &mut c.volt_watt.as_mut().unwrap().breakpoints {
                *v *= 3_f64.sqrt();
            }
        }
        let p = parameter(Kind::VoltWattBreakpoint, Some(0));
        check_fd(n.clone(), p.clone(), &outputs(&n), 0.002, |n, d| {
            perturb(n, &p, d)
        });
    }
}

fn pf_network(mut n: MulticonductorNetwork, pf: f64) -> MulticonductorNetwork {
    let mut profile = DistControlProfile::new("cp");
    profile.power_factor = Some(serde_json::from_value(json!({"pf":pf})).unwrap());
    n.control_profiles_mut().push(profile);
    let i = &mut n.ibrs_mut()[0];
    i.control_profile = Some("cp".into());
    i.q_min = Some(vec![-80.; i.s_max.len()]);
    i.q_max = Some(vec![80.; i.s_max.len()]);
    n
}
#[test]
fn leading_and_lagging_power_factor_derivatives_follow_all_coils() {
    for n in [single(), three(false), three(true)] {
        for pf in [-0.9, 0.9] {
            let n = pf_network(n.clone(), pf);
            let p = parameter(Kind::PowerFactor, None);
            check_fd(n.clone(), p.clone(), &outputs(&n), 0.00001, |n, d| {
                perturb(n, &p, d)
            });
        }
    }
}

#[test]
fn unity_power_factor_and_unused_profiles_refuse_parameter_queries() {
    for pf in [-1., 1.] {
        let mut n = pf_network(single(), pf);
        let mut unused = n.control_profiles()[0].clone();
        unused.name = "unused".into();
        n.control_profiles_mut().push(unused);
        let r = solve(n);
        let p = parameter(Kind::PowerFactor, None);
        let m = r
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == p)
            .unwrap();
        assert_eq!(m.status, McOpfMarginalStatus::UnsupportedParameter);
        let view = r.sensitivities().unwrap();
        for component in ["cp", "unused", "missing"] {
            let p = McOpfParameter {
                component: component.into(),
                ..p.clone()
            };
            assert_eq!(
                view.forward(
                    &[McOpfParameterDirection {
                        parameter: p,
                        delta: 0.
                    }],
                    &[Output::Objective]
                )
                .unwrap_err()
                .kind,
                McOpfSensitivityErrorKind::UnsupportedParameter
            );
        }
    }
}

#[test]
fn profile_mixed_derivatives_change_interior_optimal_dispatch() {
    for pf in [false, true] {
        let mut n = if pf {
            pf_network(single(), 0.95)
        } else {
            droop(Reference::SMax, false)
        };
        if !pf {
            n.control_profiles_mut()[0].volt_watt = None;
        }
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
        assert!(p > 1. && p < 149., "dispatch must be interior: {p}");
        let parameters = if pf {
            vec![parameter(Kind::PowerFactor, None)]
        } else {
            vec![
                parameter(Kind::VoltVarFraction, Some(0)),
                parameter(Kind::VoltVarBreakpoint, Some(2)),
            ]
        };
        for p in parameters {
            check_fd(n.clone(), p.clone(), &outputs(&n), 0.0001, |n, d| {
                perturb(n, &p, d)
            });
        }
    }
}

#[test]
fn exact_profile_partials_match_off_solution_residuals_and_jacobians() {
    use super::load_parameter_tests::graph;
    let compile = |n| {
        let i = McAcOpfInstance::from_network(n).unwrap();
        let prep = initial::prepare(&i, &Default::default()).unwrap();
        model::compile_with_parameters(prep, i.network()).unwrap()
    };
    let mut flat = droop(Reference::PAvailable, false);
    flat.control_profiles_mut()[0]
        .volt_var
        .as_mut()
        .unwrap()
        .q_limits = vec![0., 0.];
    flat.control_profiles_mut()[0]
        .volt_watt
        .as_mut()
        .unwrap()
        .p_limits = vec![0.5, 0.5];
    for n in [
        droop(Reference::SMax, false),
        flat,
        pf_network(single(), -0.9),
    ] {
        let m = compile(n.clone());
        let old = model::compile_with_transformer_parameters(m.prep.clone(), &n).unwrap();
        assert_eq!(m.problem.var_names, old.problem.var_names);
        assert_eq!(m.problem.con_names, old.problem.con_names);
        assert_eq!(m.problem.g_l, old.problem.g_l);
        assert_eq!(m.problem.g_u, old.problem.g_u);
        assert_eq!(m.problem.x0, old.problem.x0);
        for v in [100., 210., 220., 229., 230., 240., 350.] {
            let mut x = m.problem.x0.clone();
            for (c, name) in m.problem.var_names.iter().enumerate() {
                if name.contains(":magnitude:") {
                    x[c] = v / m.prep.bases.voltage_base_v;
                }
                if name == "Ibr:inv:0:p" {
                    x[c] = 0.037;
                }
            }
            assert_eq!(graph(&m, &x), graph(&old, &x));
            for (p, terms) in &m.dual_map.parameters {
                if p.owner != McOpfParameterOwner::ControlProfile {
                    continue;
                }
                let step = 0.0001;
                let mut plus = n.clone();
                let mut minus = n.clone();
                perturb(&mut plus, p, step);
                perturb(&mut minus, p, -step);
                let (gp, jp) = graph(&compile(plus), &x);
                let (gm, jm) = graph(&compile(minus), &x);
                for (origin, rule) in terms {
                    let duals::Rule::Control(control) = rule else {
                        panic!("profile rule");
                    };
                    let r = m
                        .dual_map
                        .rows
                        .iter()
                        .position(|r| Some(*r) == *origin)
                        .unwrap();
                    let (value, gradient) = control.evaluate(&x).unwrap();
                    close(value, (gp[r] - gm[r]) / (2. * step));
                    for c in 0..x.len() {
                        let expected = gradient
                            .iter()
                            .filter(|(col, _)| *col == c)
                            .map(|(_, v)| v)
                            .sum();
                        close(expected, (jp[r][c] - jm[r][c]) / (2. * step));
                    }
                }
            }
        }
    }
}

#[test]
fn profile_derivatives_preserve_si_units_across_working_and_objective_scales() {
    let n = droop(Reference::PAvailable, false);
    let r = solve(n.clone());
    let out = outputs(&n);
    let dir = [
        McOpfParameterDirection {
            parameter: parameter(Kind::VoltVarBreakpoint, Some(2)),
            delta: 2.,
        },
        McOpfParameterDirection {
            parameter: parameter(Kind::VoltWattFraction, Some(1)),
            delta: -0.03,
        },
    ];
    let base = r.sensitivities().unwrap().forward(&dir, &out).unwrap();
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
        close(
            base[0].derivative,
            dir.iter()
                .map(|d| marginal(&r, &d.parameter) * d.delta)
                .sum(),
        );
        for p in &parameters() {
            let unit = r
                .duals
                .as_ref()
                .unwrap()
                .marginals
                .iter()
                .find(|m| m.parameter == *p)
                .unwrap()
                .parameter_unit;
            assert_eq!(
                unit,
                if matches!(p.kind, Kind::VoltVarBreakpoint | Kind::VoltWattBreakpoint) {
                    McOpfParameterUnit::Volt
                } else {
                    McOpfParameterUnit::Ratio
                }
            );
        }
    }
}

fn edit_profile(profile: DistControlProfile) -> McOpfEdit {
    McOpfEdit::InverterControlProfile {
        profile: profile.name,
        power_factor: profile.power_factor.map(|p| p.pf),
        volt_var_breakpoints_v: profile.volt_var.as_ref().map(|v| v.breakpoints.clone()),
        volt_var_q_limits: profile.volt_var.map(|v| v.q_limits),
        volt_watt_breakpoints_v: profile.volt_watt.as_ref().map(|v| v.breakpoints.clone()),
        volt_watt_p_limits: profile.volt_watt.map(|v| v.p_limits),
    }
}

#[test]
fn retained_profile_edits_are_atomic_and_refresh_shared_derivatives() {
    let mut n = droop(Reference::PAvailable, false);
    let mut other = n.ibrs()[0].clone();
    other.name = "other".into();
    other.bus = "s".into();
    n.ibrs_mut().push(other);
    let original = n.control_profiles()[0].clone();
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(n.clone()).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    s.solve().unwrap();
    let historical = s.current_result().unwrap().result.clone();
    let p = parameter(Kind::VoltVarBreakpoint, Some(2));
    let derivative = marginal(&historical, &p);
    let mut changed = original.clone();
    changed.volt_var.as_mut().unwrap().breakpoints[2] += 0.001;
    let edit = edit_profile(changed.clone());
    let encoded = serde_json::to_string(&edit).unwrap();
    s.apply_edits(&[serde_json::from_str(&encoded).unwrap()])
        .unwrap();
    assert!(s.sensitivities().is_err());
    s.solve().unwrap();
    let plus = s.current_result().unwrap().result.clone();
    changed.volt_var.as_mut().unwrap().breakpoints[2] -= 0.002;
    s.apply_edits(&[edit_profile(changed.clone())]).unwrap();
    s.solve().unwrap();
    let minus = &s.current_result().unwrap().result;
    close(
        derivative,
        (physical(&plus, &Output::Objective) - physical(minus, &Output::Objective)) / 0.002,
    );
    assert_eq!(s.instance().network().control_profiles()[0], changed);
    let fresh = solve(s.instance().network().clone());
    close(marginal(&fresh, &p), marginal(minus, &p));
    assert!(historical.sensitivities().is_ok());
    let revision = s.revision();
    let compiled = s.compilation_count();
    for bad in [
        McOpfEdit::InverterControlProfile {
            profile: "cp".into(),
            power_factor: Some(0.9),
            volt_var_breakpoints_v: None,
            volt_var_q_limits: None,
            volt_watt_breakpoints_v: None,
            volt_watt_p_limits: None,
        },
        {
            let mut c = changed.clone();
            c.volt_var.as_mut().unwrap().breakpoints[1] = 0.;
            edit_profile(c)
        },
        {
            let mut c = changed.clone();
            c.volt_var.as_mut().unwrap().breakpoints.reverse();
            edit_profile(c)
        },
        {
            let mut c = changed.clone();
            c.volt_watt.as_mut().unwrap().p_limits = vec![0.1];
            edit_profile(c)
        },
        {
            let mut c = changed.clone();
            c.volt_watt.as_mut().unwrap().p_limits[0] = f64::NAN;
            edit_profile(c)
        },
        {
            let mut c = changed.clone();
            c.name = "missing".into();
            edit_profile(c)
        },
    ] {
        assert!(s
            .apply_edits(&[edit_profile(original.clone()), bad])
            .is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(s.compilation_count(), compiled);
        assert_eq!(s.instance().network().control_profiles()[0], changed);
        assert!(s.current_result().is_some());
        assert!(s.sensitivities().is_ok());
    }
}

#[test]
fn retained_power_factor_edits_preserve_families_and_reject_invalid_values() {
    let n = pf_network(single(), 0.9);
    let mut profile = n.control_profiles()[0].clone();
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(n).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    for pf in [-0.9, -1., 1., 0.95] {
        profile.power_factor.as_mut().unwrap().pf = pf;
        s.apply_edits(&[edit_profile(profile.clone())]).unwrap();
        s.solve().unwrap();
        let r = &s.current_result().unwrap().result;
        let fresh = solve(s.instance().network().clone());
        close(
            physical(r, &Output::Objective),
            physical(&fresh, &Output::Objective),
        );
        assert_eq!(s.instance().network().control_profiles()[0], profile);
    }
    let revision = s.revision();
    for pf in [0., 1.01, -1.01, f64::NAN, f64::INFINITY] {
        let mut bad = profile.clone();
        bad.power_factor.as_mut().unwrap().pf = pf;
        assert!(s.apply_edits(&[edit_profile(bad)]).is_err());
        assert_eq!(s.revision(), revision);
        assert_eq!(s.instance().network().control_profiles()[0], profile);
        assert!(s.current_result().is_some());
    }
    let unchanged: McOpfEdit = serde_json::from_value(json!({
        "edit": "inverter_control_profile", "profile": "cp"
    }))
    .unwrap();
    s.apply_edits(&[unchanged]).unwrap();
    assert_eq!(s.instance().network().control_profiles()[0], profile);
}

#[test]
fn current_version_roundtrips_verify_legacy_maps_before_upgrading_profile_claims() {
    let r = solve(droop(Reference::PAvailable, false));
    let prep = initial::prepare(
        r.solution.instance(),
        &McAcOpfAssemblyOptions::new(r.options.voltage_base_v, r.options.power_base_va),
    )
    .unwrap();
    let legacy =
        model::compile_with_transformer_parameters(prep, r.solution.instance().network()).unwrap();
    let d = r.duals.as_ref().unwrap();
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
        .all(|m| m.parameter.owner != McOpfParameterOwner::ControlProfile));
    let saved = r.to_module_json().unwrap();
    let read = McOpfResult::from_module_json(&saved).unwrap();
    let p = parameter(Kind::VoltVarBreakpoint, Some(2));
    close(marginal(&read, &p), marginal(&r, &p));
    let mut module = crate::ir::deserialize_module(&saved).unwrap();
    let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
    assert_eq!(report["version"], json!(7));
    report["version"] = json!(5);
    report["duals"] = serde_json::to_value(old).unwrap();
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    let read =
        McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
    close(marginal(&read, &p), marginal(&r, &p));
    assert!(read.sensitivities().is_ok());
    assert_eq!(
        crate::ir::deserialize_module(&read.to_module_json().unwrap())
            .unwrap()
            .extensions()[MC_OPF_RESULT_EXTENSION]["version"],
        json!(7)
    );
    report["version"] = json!(7);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
    report["version"] = json!(5);
    report["duals"]["constraints"][0]["multiplier"] = json!(987.);
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report)
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
}
