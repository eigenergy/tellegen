use super::sensitivity_tests::{physical, solve, solve_options};
use super::*;
use powerio::MulticonductorNetwork;
use powerio_dist::DistLine;
use serde_json::json;

fn feeder(sections: usize) -> MulticonductorNetwork {
    let mut n = tests::network();
    let bus = n.buses()[1].clone();
    let load = n.loads()[0].clone();
    n.buses_mut().truncate(1);
    n.loads_mut().clear();
    n.lines_mut().clear();
    for i in 1..=sections {
        let name = format!("b{i}");
        let mut b = bus.clone();
        b.id = name.clone();
        n.buses_mut().push(b);
        let mut d = load.clone();
        d.name = format!("d{i}");
        d.bus = name.clone();
        d.p_nom[0] = 2.;
        d.q_nom[0] = 0.4;
        n.loads_mut().push(d);
        let from = if i == 1 {
            "s".to_string()
        } else {
            format!("b{}", i - 1)
        };
        n.lines_mut().push(DistLine::new(
            format!("l{i}"),
            from,
            name,
            vec!["a".into(), "n".into()],
            vec!["a".into(), "n".into()],
            "c",
            1.,
        ));
    }
    n
}
fn parameter() -> McOpfParameter {
    McOpfParameter::new(
        McOpfParameterOwner::Load,
        "d48",
        Some(0),
        McOpfParameterKind::LoadActivePower,
    )
}
fn outputs() -> Vec<McOpfSensitivityOutput> {
    vec![
        McOpfSensitivityOutput::Objective,
        McOpfSensitivityOutput::Voltage {
            terminal: McOpfTerminalId {
                bus: "b48".into(),
                terminal: "a".into(),
            },
            part: McOpfVoltagePart::Real,
        },
    ]
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8 + 5e-4 * b.abs(), "{a} != {b}");
}

#[test]
fn thirty_bus_feeder_qualifies_with_a_bounded_ci_budget() {
    let start = std::time::Instant::now();
    let n = feeder(29);
    assert_eq!(n.buses().len(), 30);
    // One solve and one sensitivity factorization; no perturbation sweep.
    let r = solve_options(
        n,
        McOpfOptions {
            collect_duals: true,
            max_iterations: 100,
            ..Default::default()
        },
    );
    let d = r.duals.as_ref().unwrap();
    assert_eq!(
        d.status,
        McOpfMarginalStatus::Conditional,
        "{:?}",
        d.active_rank
    );
    assert!(d.active_rank.active_rows > 256);
    assert!(d.variables.len() < 1000);
    let q = d.active_rank.qualification.as_ref().unwrap();
    assert_eq!(q.outcome, McOpfRankOutcome::Complete);
    assert!(q.work < 5_000_000, "{q:?}");
    assert!(q.peak_nonzeros < 50_000, "{q:?}");
    let parameter = McOpfParameter::new(
        McOpfParameterOwner::Load,
        "d29",
        Some(0),
        McOpfParameterKind::LoadActivePower,
    );
    let view = r.sensitivities().unwrap();
    assert!(view.diagnostics().kkt_nonzeros < 20 * view.diagnostics().kkt_dimension);
    let (forward, fcheck) = view
        .forward_with_diagnostics(
            &[McOpfParameterDirection {
                parameter: parameter.clone(),
                delta: 1.,
            }],
            &[McOpfSensitivityOutput::Objective],
        )
        .unwrap();
    let (adjoint, acheck) = view
        .adjoint_with_diagnostics(
            &[McOpfOutputWeight {
                output: McOpfSensitivityOutput::Objective,
                weight: 1.,
            }],
            std::slice::from_ref(&parameter),
        )
        .unwrap();
    close(forward[0].derivative, adjoint[0].derivative);
    close(
        forward[0].derivative,
        d.marginals
            .iter()
            .find(|m| m.parameter == parameter)
            .unwrap()
            .derivative
            .unwrap(),
    );
    assert!(fcheck.residual_tolerance_ratio <= 1.);
    assert!(acheck.residual_tolerance_ratio <= 1.);
    eprintln!(
        "30-bus rank CI: {} rows, {} work, {} peak nonzeros, {:?}",
        d.active_rank.active_rows,
        q.work,
        q.peak_nonzeros,
        start.elapsed()
    );
}

#[test]
#[ignore = "opt-in 48-section study; run mc_opf::rank_tests with --ignored"]
fn connected_feeder_qualifies_beyond_old_limits_and_predicts_perturbed_solves() {
    let n = feeder(48);
    let r = solve(n.clone());
    let d = r.duals.as_ref().unwrap();
    assert_eq!(
        d.status,
        McOpfMarginalStatus::Conditional,
        "{:?}",
        d.active_rank
    );
    assert!(d.active_rank.active_rows > 256);
    assert!(d.active_rank.active_rows * d.variables.len() > 262_144);
    assert_eq!(d.active_rank.rank, Some(d.active_rank.active_rows));
    let view = r.sensitivities().unwrap();
    let stats = view.diagnostics();
    assert_eq!(
        stats.kkt_dimension,
        stats.variables + stats.active_rank.active_rows
    );
    assert!(stats.kkt_nonzeros < stats.kkt_dimension * 20);
    let direction = [McOpfParameterDirection {
        parameter: parameter(),
        delta: 1.,
    }];
    let out = outputs();
    let (forward, residual) = view.forward_with_diagnostics(&direction, &out).unwrap();
    assert!(residual.residual_tolerance_ratio <= 1.);
    // Near-zero rows can be governed by the absolute residual allowance.
    assert!(residual.componentwise_backward_error.is_finite());
    let mut plus = n.clone();
    let mut minus = n.clone();
    plus.loads_mut()[47].p_nom[0] += 0.01;
    minus.loads_mut()[47].p_nom[0] -= 0.01;
    let plus = solve(plus);
    let minus = solve(minus);
    for (value, output) in forward.iter().zip(&out) {
        close(
            value.derivative,
            (physical(&plus, output) - physical(&minus, output)) / 0.02,
        );
        let (adjoint, residual) = view
            .adjoint_with_diagnostics(
                &[McOpfOutputWeight {
                    output: output.clone(),
                    weight: 1.,
                }],
                &[parameter()],
            )
            .unwrap();
        close(value.derivative, adjoint[0].derivative);
        assert!(residual.residual_tolerance_ratio <= 1.);
    }
    let marginal = d
        .marginals
        .iter()
        .find(|m| m.parameter == parameter())
        .unwrap();
    close(forward[0].derivative, marginal.derivative.unwrap());
    let mut reordered = n;
    reordered.buses_mut().reverse();
    let scaled = solve_options(
        reordered,
        McOpfOptions {
            voltage_base_v: 100.,
            power_base_va: 4000.,
            objective_scale: 0.2,
            collect_duals: true,
            ..Default::default()
        },
    );
    for (a, b) in scaled
        .sensitivities()
        .unwrap()
        .forward(&direction, &out)
        .unwrap()
        .iter()
        .zip(&forward)
    {
        close(a.derivative, b.derivative);
    }
}

#[test]
#[ignore = "opt-in 48-section report study; run mc_opf::rank_tests with --ignored"]
fn legacy_rank_refusals_are_verified_before_upgrade_and_new_claims_are_checked() {
    check_rank_report(feeder(48), McOpfMarginalStatus::RankNotChecked);
}

#[test]
fn compact_rank_reports_keep_legacy_verification_and_tamper_checks_in_ci() {
    check_rank_report(tests::network(), McOpfMarginalStatus::Conditional);
}

fn check_rank_report(network: MulticonductorNetwork, legacy_status: McOpfMarginalStatus) {
    let r = solve(network);
    let d = r.duals.as_ref().unwrap();
    let model = r.verified_dual_model().unwrap().unwrap();
    let lambda: Vec<_> = d.constraints.iter().map(|c| c.multiplier).collect();
    let old = duals::analyze_legacy(
        &model,
        &d.primal,
        &lambda,
        &d.lower_bound_multipliers,
        &d.upper_bound_multipliers,
    )
    .unwrap();
    assert_eq!(old.status, legacy_status);
    let saved = r.to_module_json().unwrap();
    let mut module = crate::ir::deserialize_module(&saved).unwrap();
    let current = module.extensions()[MC_OPF_RESULT_EXTENSION].clone();
    assert_eq!(current["version"], json!(9));
    let old_sparse = duals::analyze_inner(
        &model,
        &d.primal,
        &lambda,
        &d.lower_bound_multipliers,
        &d.upper_bound_multipliers,
        8,
    )
    .unwrap();
    for version in [7, 8] {
        let mut report = current.clone();
        report["version"] = json!(version);
        report["duals"] = serde_json::to_value(&old_sparse).unwrap();
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
            .unwrap();
        let read =
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
        assert_eq!(read.duals.as_ref().unwrap().active_rank, d.active_rank);
        let exported = crate::ir::deserialize_module(&read.to_module_json().unwrap()).unwrap();
        assert_eq!(
            exported.extensions()[MC_OPF_RESULT_EXTENSION]["version"],
            json!(9)
        );
        // Verification must use the old counters, not trust or overwrite them.
        report["duals"]["active_rank"]["qualification"]["work"] = json!(0);
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report)
            .unwrap();
        assert!(
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err()
        );
    }
    let mut report = current.clone();
    report["version"] = json!(6);
    report["duals"] = serde_json::to_value(old).unwrap();
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report.clone())
        .unwrap();
    let upgraded =
        McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).unwrap();
    assert_eq!(
        upgraded.duals.as_ref().unwrap().status,
        McOpfMarginalStatus::Conditional
    );
    assert!(upgraded.sensitivities().is_ok());
    report["duals"]["status"] = json!("rank_deficient");
    module
        .insert_extension(MC_OPF_RESULT_EXTENSION, report)
        .unwrap();
    assert!(McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err());
    for (field, value) in [
        ("work", json!(0)),
        ("outcome", json!("work_limit")),
        ("tolerance", json!(0.01)),
    ] {
        let mut report = current.clone();
        report["duals"]["active_rank"]["qualification"][field] = value;
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, report)
            .unwrap();
        assert!(
            McOpfResult::from_module_json(&crate::ir::serialize_module(&module).unwrap()).is_err()
        );
    }
    let read = McOpfResult::from_module_json(&saved).unwrap();
    assert!(read.sensitivities().is_ok());
}

#[test]
fn diagnostic_queries_preserve_session_results_and_reject_stale_views() {
    let mut session = McOpfSession::new(
        McAcOpfInstance::from_network(tests::network()).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    session.solve().unwrap();
    let before = session
        .current_result()
        .unwrap()
        .result
        .to_module_json()
        .unwrap();
    {
        let view = session.sensitivities().unwrap();
        let (zero, diagnostics) = view
            .forward_with_diagnostics(&[], &[McOpfSensitivityOutput::Objective])
            .unwrap();
        assert_eq!(zero[0].derivative, 0.);
        assert_eq!(diagnostics.residual_inf, 0.);
        assert_eq!(diagnostics.refinement_steps, 0);
        serde_json::to_string(&view.diagnostics()).unwrap();
        assert!(view
            .forward_with_diagnostics(
                &[McOpfParameterDirection {
                    parameter: parameter(),
                    delta: 0.
                }],
                &[]
            )
            .is_err());
    }
    assert_eq!(
        session
            .current_result()
            .unwrap()
            .result
            .to_module_json()
            .unwrap(),
        before
    );
    session
        .apply_edits(&[McOpfEdit::LoadPower {
            load: "d".into(),
            p_w: vec![110.],
            q_var: vec![0.],
        }])
        .unwrap();
    assert_eq!(
        session.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
    session.solve().unwrap();
    assert!(session
        .sensitivities()
        .unwrap()
        .diagnostics()
        .active_rank
        .qualification
        .is_some());
}
