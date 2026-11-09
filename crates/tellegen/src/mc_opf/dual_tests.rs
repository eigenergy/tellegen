use super::*;
use powerio::MulticonductorNetwork;
use McOpfMarginalStatus as Status;
use McOpfParameterKind as Kind;
use McOpfParameterOwner as Owner;

fn solve(net: MulticonductorNetwork) -> McOpfResult {
    solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net).unwrap()),
        &McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap()
}
fn cost(net: MulticonductorNetwork) -> f64 {
    solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net).unwrap()),
        &Default::default(),
    )
    .unwrap()
    .solution
    .objective()
}
fn value(r: &McOpfResult, owner: Owner, component: &str, axis: Option<usize>, kind: Kind) -> f64 {
    let d = r.duals.as_ref().unwrap();
    let entry = d
        .marginals
        .iter()
        .find(|m| m.parameter == McOpfParameter::new(owner, component, axis, kind))
        .unwrap_or_else(|| panic!("missing parameter {component} {kind:?}: {:?}", d.marginals));
    assert_eq!(
        entry.status,
        Status::Conditional,
        "{:?} {:?}",
        d.kkt,
        d.active_rank
    );
    entry.derivative.unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 2e-10 + 1e-3 * b.abs(), "{a} != {b}");
}
fn one_bus(generator: bool) -> MulticonductorNetwork {
    let mut net = if generator {
        tests::rich_network()
    } else {
        tests::network()
    };
    net.lines_mut().clear();
    net.line_codes_mut().clear();
    net.buses_mut().truncate(1);
    net.shunts_mut().clear();
    for d in net.loads_mut() {
        d.bus = "s".into();
    }
    for d in net.generators_mut() {
        d.bus = "s".into();
    }
    net
}

#[test]
fn duals_are_opt_in_and_physical_success_survives_diagnostic_limits() {
    let instance = Arc::new(McAcOpfInstance::from_network(tests::network()).unwrap());
    let cold = solve_mc_ac_opf_instance(instance.clone(), &Default::default()).unwrap();
    assert!(cold.duals.is_none());
    let options = McOpfOptions {
        collect_duals: true,
        ..Default::default()
    };
    let result = solve_mc_ac_opf_instance(instance, &options).unwrap();
    let d = result.duals.as_ref().expect("dual report");
    assert_eq!(d.status, Status::Conditional);
    assert!(d.kkt.stationarity_relative_inf < 1e-6);
    assert!((result.solution.objective() - cold.solution.objective()).abs() < 1e-10);
    McOpfResult::from_module_json(&result.to_module_json().unwrap()).unwrap();
}

#[test]
fn analytic_load_marginals_have_physical_units_and_scale_invariance() {
    let mut net = one_bus(false);
    net.loads_mut()[0].q_nom[0] = 20.;
    for (vb, sb, objective_scale) in [(230., 1000., 1.), (100., 4000., 0.2), (400., 700., 7.)] {
        let options = McOpfOptions {
            voltage_base_v: vb,
            power_base_va: sb,
            objective_scale,
            collect_duals: true,
            ..Default::default()
        };
        let r = solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(net.clone()).unwrap()),
            &options,
        )
        .unwrap();
        close(
            value(&r, Owner::Load, "d", Some(0), Kind::LoadActivePower),
            0.2 / 1000.,
        );
        close(
            value(&r, Owner::Load, "d", Some(0), Kind::LoadReactivePower),
            0.,
        );
        assert!(r.duals.as_ref().unwrap().kkt.complementarity_relative_inf < 1e-6);
        assert_eq!(
            r.duals.as_ref().unwrap().marginals[0].parameter_unit,
            McOpfParameterUnit::Watt
        );
    }
}

#[test]
fn generator_upper_lower_inactive_and_coincident_bounds_are_distinguished() {
    let net = one_bus(true);
    let r = solve(net.clone());
    close(
        value(&r, Owner::Generator, "g", Some(0), Kind::ActivePowerUpper),
        -(0.2 - 0.05) / 1000.,
    );
    close(
        value(&r, Owner::Generator, "g", Some(0), Kind::ActivePowerLower),
        0.,
    );
    let q = r
        .duals
        .as_ref()
        .unwrap()
        .marginals
        .iter()
        .find(|m| m.parameter.kind == Kind::ReactivePowerUpper)
        .unwrap();
    assert_eq!(q.status, Status::CoincidentBounds);
    assert!(q.derivative.is_none());
    let mut plus = net.clone();
    let mut minus = net.clone();
    plus.generators_mut()[0].p_max = Some(vec![50.1]);
    minus.generators_mut()[0].p_max = Some(vec![49.9]);
    close(
        value(&r, Owner::Generator, "g", Some(0), Kind::ActivePowerUpper),
        (cost(plus) - cost(minus)) / 0.2,
    );
    let mut lower = net;
    lower.generators_mut()[0].cost = Some(vec![1.]);
    lower.generators_mut()[0].p_min = Some(vec![20.]);
    close(
        value(
            &solve(lower),
            Owner::Generator,
            "g",
            Some(0),
            Kind::ActivePowerLower,
        ),
        (1. - 0.2) / 1000.,
    );
}

#[test]
fn lossy_load_p_and_q_marginals_match_cost_perturbations() {
    let mut net = tests::network();
    net.loads_mut()[0].q_nom[0] = 30.;
    net.line_codes_mut()[0].x_series = vec![vec![0.05, 0.01], vec![0.01, 0.05]];
    let r = solve(net.clone());
    for reactive in [false, true] {
        let mut plus = net.clone();
        let mut minus = net.clone();
        if reactive {
            plus.loads_mut()[0].q_nom[0] += 0.1;
            minus.loads_mut()[0].q_nom[0] -= 0.1;
        } else {
            plus.loads_mut()[0].p_nom[0] += 0.1;
            minus.loads_mut()[0].p_nom[0] -= 0.1;
        }
        close(
            value(
                &r,
                Owner::Load,
                "d",
                Some(0),
                if reactive {
                    Kind::LoadReactivePower
                } else {
                    Kind::LoadActivePower
                },
            ),
            (cost(plus) - cost(minus)) / 0.2,
        );
    }
}
fn constrained_network() -> MulticonductorNetwork {
    let mut net = tests::rich_network();
    net.generators_mut()[0].cost = Some(vec![1.]);
    net.generators_mut()[0].p_max = Some(vec![150.]);
    net
}
#[test]
fn normalized_branch_ratings_sum_end_contributions_and_match_perturbations() {
    for apparent in [false, true] {
        let mut net = constrained_network();
        if apparent {
            net.lines_mut()[0].s_max = Some(vec![46., 1000.]);
        } else {
            net.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
        }
        let r = solve(net.clone());
        let mut plus = net.clone();
        let mut minus = net;
        let step = if apparent { 0.01 } else { 0.0001 };
        if apparent {
            plus.lines_mut()[0].s_max.as_mut().unwrap()[0] += step;
            minus.lines_mut()[0].s_max.as_mut().unwrap()[0] -= step;
        } else {
            plus.lines_mut()[0].i_max.as_mut().unwrap()[0] += step;
            minus.lines_mut()[0].i_max.as_mut().unwrap()[0] -= step;
        }
        let kind = if apparent {
            Kind::ApparentPowerRating
        } else {
            Kind::CurrentRating
        };
        let marginal = value(&r, Owner::Branch, "line:l", Some(0), kind);
        assert!(marginal < -1e-5);
        close(marginal, (cost(plus) - cost(minus)) / (2. * step));
        let entry = r
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter.kind == kind && m.parameter.axis == Some(0))
            .unwrap();
        assert_eq!(entry.row_ordinals.len(), 2);
    }
}
#[test]
fn voltage_floor_marginal_accounts_for_normalized_radius() {
    let mut net = constrained_network();
    net.buses_mut()[1].vpn_min = Some(vec![229.96]);
    let r = solve(net.clone());
    let d = r.duals.as_ref().unwrap();
    let m = d
        .marginals
        .iter()
        .find(|m| m.parameter.kind == Kind::VoltageLower && m.derivative.is_some_and(|v| v > 1e-5))
        .unwrap_or_else(|| panic!("{:?}", d.marginals));
    let mut plus = net.clone();
    let mut minus = net;
    plus.buses_mut()[1].vpn_min = Some(vec![229.9601]);
    minus.buses_mut()[1].vpn_min = Some(vec![229.9599]);
    close(m.derivative.unwrap(), (cost(plus) - cost(minus)) / 0.0002);
}

#[test]
fn binding_rating_marginals_are_invariant_to_working_and_objective_scales() {
    let mut net = constrained_network();
    net.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
    let base = solve(net.clone());
    let expected = value(&base, Owner::Branch, "line:l", Some(0), Kind::CurrentRating);
    for (vb, sb, objective_scale) in [(100., 4000., 0.2), (400., 700., 7.)] {
        let r = solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(net.clone()).unwrap()),
            &McOpfOptions {
                voltage_base_v: vb,
                power_base_va: sb,
                objective_scale,
                collect_duals: true,
                ..Default::default()
            },
        )
        .unwrap();
        close(
            value(&r, Owner::Branch, "line:l", Some(0), Kind::CurrentRating),
            expected,
        );
        close(
            value(&r, Owner::Branch, "line:l", Some(1), Kind::CurrentRating),
            0.,
        );
    }
}
#[test]
fn delta_coil_marginals_follow_identity_under_reordering() {
    let mut net = initial_tests::case("components", "capacitor_delta");
    net.generators_mut().clear();
    net.buses_mut().reverse();
    net.loads_mut().reverse();
    for bus in net.buses_mut() {
        bus.terminals.reverse();
    }
    let index = net
        .loads()
        .iter()
        .position(|l| l.configuration == powerio_dist::Configuration::Delta)
        .unwrap();
    let name = net.loads()[index].name.clone();
    let r = solve(net.clone());
    let mut plus = net.clone();
    let mut minus = net;
    plus.loads_mut()[index].p_nom[1] += 0.1;
    minus.loads_mut()[index].p_nom[1] -= 0.1;
    close(
        value(&r, Owner::Load, &name, Some(1), Kind::LoadActivePower),
        (cost(plus) - cost(minus)) / 0.2,
    );
}
#[test]
fn zero_reductions_keep_provenance_without_fabricating_prices() {
    let mut net = tests::network();
    net.loads_mut()[0].p_nom[0] = 0.;
    net.lines_mut()[0].i_max = Some(vec![0.; 2]);
    let r = solve(net);
    let d = r.duals.as_ref().unwrap();
    assert_eq!(d.status, Status::ZeroLimitReduction);
    assert!(d.marginals.iter().all(|m| m.derivative.is_none()));
    assert!(d
        .omitted
        .iter()
        .any(|o| o.reason == McOpfRowOmission::ZeroReduction));
    assert!(d.constraints.iter().any(|r| r.identity.synthetic));
    let all: std::collections::BTreeSet<_> = d
        .constraints
        .iter()
        .map(|r| r.identity.ordinal)
        .chain(d.omitted.iter().map(|o| o.identity.ordinal))
        .collect();
    assert_eq!(all.len(), d.constraints.len() + d.omitted.len());
    McOpfResult::from_module_json(&r.to_module_json().unwrap()).unwrap();
}
#[test]
fn redundant_binding_rows_report_multiplier_nonuniqueness_risk() {
    let mut net = constrained_network();
    let code = &mut net.line_codes_mut()[0];
    code.g_from = vec![vec![0.; 2]; 2];
    code.b_to = vec![vec![0.; 2]; 2];
    net.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
    let r = solve(net);
    let d = r.duals.as_ref().unwrap();
    assert_eq!(
        d.status,
        Status::RankDeficient,
        "{:?} {:?}",
        d.kkt,
        d.active_rank
    );
    assert!(d.marginals.iter().all(|m| m.derivative.is_none()));
    assert!(r.residuals.maximum() < 1e-6);
}
#[test]
fn session_edits_refresh_duals_without_stale_current_results() {
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(tests::network()).unwrap(),
        Default::default(),
    )
    .unwrap();
    s.solve().unwrap();
    assert!(s.current_result().unwrap().result.duals.is_none());
    let mut options = s.options().clone();
    options.collect_duals = true;
    s.set_options(options).unwrap();
    assert_eq!(s.compilation_count(), 1);
    s.solve().unwrap();
    let old = s.current_result().unwrap().result.duals.clone();
    s.apply_edits(&[McOpfEdit::LoadPower {
        load: "d".into(),
        p_w: vec![200.],
        q_var: vec![0.],
    }])
    .unwrap();
    assert!(s.current_result().is_none());
    assert_eq!(s.last_accepted().unwrap().result.duals, old);
    s.solve().unwrap();
    let r = &s.current_result().unwrap().result;
    let fresh = solve(s.instance().network().clone());
    close(
        value(r, Owner::Load, "d", Some(0), Kind::LoadActivePower),
        value(&fresh, Owner::Load, "d", Some(0), Kind::LoadActivePower),
    );
}

#[test]
fn saved_duals_reject_changed_coordinates_scaling_and_derived_claims() {
    use serde_json::json;
    let r = solve(tests::network());
    let saved = r.to_module_json().unwrap();
    for change in 0..10 {
        let mut module = crate::ir::deserialize_module(&saved).unwrap();
        let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
        let d = &mut report["duals"];
        match change {
            0 => d["constraints"][0]["identity"]["ordinal"] = json!(9999),
            1 => d["constraints"][0]["identity"]["norm_divisor_pu"] = json!(42.),
            2 => d["constraints"][0]["multiplier"] = json!(123.),
            3 => d["marginals"][0]["derivative"] = json!(0.5),
            4 => d["primal"] = json!([]),
            5 => d["status"] = json!("rank_deficient"),
            6 => d["lower_bound_multipliers"][0] = json!(null),
            7 => report["version"] = json!(1),
            8 => report["options"]["collect_duals"] = json!(false),
            9 => report["devices"][0]["terminal_current_a"][0][0] = json!(99.),
            _ => unreachable!(),
        }
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report)
            .unwrap();
        let e = McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap())
            .unwrap_err();
        assert_eq!(e.stage, McOpfStage::Import, "change {change}");
    }
    let mut nonfinite = r.clone();
    nonfinite.duals.as_mut().unwrap().marginals[0].derivative = Some(f64::NAN);
    assert!(nonfinite.to_module_json().is_err());
    let mut hidden_nan = r;
    hidden_nan
        .duals
        .as_mut()
        .unwrap()
        .constraints
        .iter_mut()
        .find(|r| r.lower.is_none())
        .unwrap()
        .lower = Some(f64::NAN);
    assert!(hidden_nan.to_module_json().is_err());
}

#[test]
fn legacy_version_one_reports_without_duals_remain_readable() {
    use serde_json::json;
    let r = solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(tests::network()).unwrap()),
        &Default::default(),
    )
    .unwrap();
    let mut module = crate::ir::deserialize_module(&r.to_module_json().unwrap()).unwrap();
    let mut report = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
    report["version"] = json!(1);
    report.as_object_mut().unwrap().remove("duals");
    report["options"]
        .as_object_mut()
        .unwrap()
        .remove("collect_duals");
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report)
        .unwrap();
    let read =
        McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
    assert!(read.duals.is_none());
    assert!(!read.options.collect_duals);
}

#[test]
fn voltage_dependent_nominal_load_parameters_retain_zero_coefficients() {
    use powerio_dist::DistLoadVoltageModel as Law;
    for law in [
        Law::ConstantImpedance { v_nom: vec![230.] },
        // Q_nom = 0 removes its terms during preparation. The resulting
        // constant-looking row must not imply a constant-power Q derivative.
        Law::Exponential {
            v_nom: vec![230.],
            gamma_p: vec![0.],
            gamma_q: vec![2.],
        },
    ] {
        let mut net = tests::network();
        net.loads_mut()[0].voltage_model = law;
        let r = solve(net);
        let d = r.duals.unwrap();
        assert_eq!(d.status, Status::Conditional);
        let loads: Vec<_> = d
            .marginals
            .iter()
            .filter(|m| m.parameter.owner == Owner::Load)
            .collect();
        assert_eq!(loads.len(), 2);
        for m in loads {
            assert_eq!(m.status, Status::Conditional);
            assert!(m.derivative.is_some());
        }
    }
}

// Analytic optimization examples exercise the diagnostic contract independently
// of electrical feasibility and the optimizer's choice among nonunique duals.
fn quadratic_model(rows: usize, equality: bool) -> model::Model {
    use pounce_nl::nl_reader::{BinOp, Expr, NlProblem, NlProblemParts};
    let instance = McAcOpfInstance::from_network(tests::network()).unwrap();
    let mut m =
        model::compile(build_mc_ac_opf_preparation(&instance, &Default::default()).unwrap())
            .unwrap();
    m.problem = NlProblem::from_expressions(NlProblemParts {
        minimize: true,
        objective: Expr::Binary(BinOp::Mul, Box::new(Expr::Var(0)), Box::new(Expr::Var(0))),
        obj_constant: 0.,
        constraints: vec![Expr::Var(0); rows],
        x_l: vec![f64::NEG_INFINITY],
        x_u: vec![f64::INFINITY],
        x0: vec![0.],
        g_l: vec![0.; rows],
        g_u: vec![if equality { 0. } else { f64::INFINITY }; rows],
        var_names: vec!["x".into()],
        con_names: vec!["row".into(); rows],
    })
    .unwrap();
    m.dual_map = duals::CompilerMap::default();
    for _ in 0..rows {
        let id = m.dual_map.origin("row".into());
        m.dual_map.rows.push(id);
    }
    m
}

#[test]
fn analytic_kkt_checks_distinguish_weak_activity_bad_duals_and_nonuniqueness() {
    let m = quadratic_model(1, false);
    let weak = duals::analyze(&m, &[0.], &[0.], &[0.], &[0.]).unwrap();
    assert_eq!(weak.status, Status::WeakActiveSet);
    assert_eq!(weak.active_rank.rank, Some(1));
    let wrong = duals::analyze(&m, &[0.], &[1.], &[0.], &[0.]).unwrap();
    assert_eq!(wrong.status, Status::FailedKktCheck);
    assert_eq!(wrong.kkt.dual_feasibility_inf, 1.);
    assert_eq!(wrong.kkt.stationarity_inf, 1.);
    let complementarity = duals::analyze(&m, &[1.], &[-2.], &[0.], &[0.]).unwrap();
    assert_eq!(complementarity.kkt.stationarity_inf, 0.);
    assert_eq!(complementarity.kkt.complementarity_inf, 2.);
    assert_eq!(complementarity.status, Status::FailedKktCheck);
    let duplicate =
        duals::analyze(&quadratic_model(2, true), &[0.], &[1., -1.], &[0.], &[0.]).unwrap();
    assert_eq!(duplicate.status, Status::RankDeficient);
    assert_eq!(duplicate.kkt.stationarity_inf, 0.);
    assert!(duals::analyze(&m, &[0.], &[f64::NAN], &[0.], &[0.]).is_err());
    assert!(duals::analyze(&m, &[], &[0.], &[0.], &[0.]).is_err());
}

#[test]
fn large_dependent_active_sets_are_qualified_instead_of_skipped() {
    let d = duals::analyze(
        &quadratic_model(257, true),
        &[0.],
        &vec![0.; 257],
        &[0.],
        &[0.],
    )
    .unwrap();
    assert_eq!(d.status, Status::RankDeficient);
    assert_eq!(d.active_rank.rank, Some(1));
    assert_eq!(
        d.active_rank.qualification.unwrap().outcome,
        McOpfRankOutcome::Complete
    );
    assert_eq!(d.constraints.len(), 257);
}
