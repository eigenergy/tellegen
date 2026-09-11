//! Moreau adapters for the existing sparse conic program and original solution coordinates.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::problem::OpfProgram;
use crate::solve::{RawSolution, SolveIteration};
use moreau::algebra::CscMatrix;
use moreau::solver::{
    DefaultInfo, DefaultSettings, DefaultSolver, IPSolver, SolverStatus, SupportedConeT,
};

pub(crate) fn matrix(value: &clarabel::algebra::CscMatrix<f64>) -> CscMatrix<f64> {
    CscMatrix::new(
        value.m,
        value.n,
        value.colptr.clone(),
        value.rowval.clone(),
        value.nzval.clone(),
    )
}

pub(crate) fn cones(program: &OpfProgram) -> Result<Vec<SupportedConeT<f64>>, String> {
    program
        .cones
        .iter()
        .map(|cone| match cone {
            clarabel::solver::SupportedConeT::ZeroConeT(n) => Ok(SupportedConeT::ZeroConeT(*n)),
            clarabel::solver::SupportedConeT::NonnegativeConeT(n) => {
                Ok(SupportedConeT::NonnegativeConeT(*n))
            }
            clarabel::solver::SupportedConeT::SecondOrderConeT(n) => {
                Ok(SupportedConeT::SecondOrderConeT(*n))
            }
            _ => Err("unsupported cone in Moreau OPF adapter".into()),
        })
        .collect()
}

pub(crate) fn solve(
    prog: &OpfProgram,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<RawSolution, String> {
    if cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) {
        return Err("DC OPF solve cancelled".into());
    }
    let mut settings = DefaultSettings {
        verbose: false,
        ..DefaultSettings::default()
    };
    settings.ipm.direct_solve_method = "qdldl".into();
    settings.ipm.tol_gap_abs = 1e-9;
    settings.ipm.tol_gap_rel = 1e-9;
    settings.ipm.tol_feas = 1e-9;
    let mut solver = DefaultSolver::new(
        &matrix(&prog.p),
        &prog.q,
        &matrix(&prog.a),
        &prog.b,
        &cones(prog)?,
        settings,
    )
    .map_err(|e| format!("Moreau setup failed: {e:?}"))?;
    let trace: Arc<Mutex<Vec<SolveIteration>>> = Arc::new(Mutex::new(Vec::new()));
    let iterations = trace.clone();
    solver.set_termination_callback(move |info: &DefaultInfo<f64>| {
        iterations.lock().unwrap().push(SolveIteration {
            iter: info.iterations,
            objective: info.cost_primal,
            inf_pr: info.res_primal,
            inf_du: info.res_dual,
        });
        cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
    });
    solver.solve();
    match solver.solution.status {
        SolverStatus::Solved | SolverStatus::AlmostSolved => {}
        SolverStatus::CallbackTerminated => return Err("DC OPF solve cancelled".into()),
        SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible => {
            return Err("DC OPF solve infeasible (Moreau)".into())
        }
        SolverStatus::DualInfeasible | SolverStatus::AlmostDualInfeasible => {
            return Err("DC OPF solve unbounded (Moreau)".into())
        }
        status => {
            return Err(format!(
                "DC OPF solve did not converge (Moreau): {status:?}"
            ))
        }
    }
    let iterations = std::mem::take(&mut *trace.lock().unwrap());
    Ok(RawSolution {
        x: solver.solution.x.clone(),
        z: solver.solution.z.clone(),
        objective: solver.solution.obj_val,
        iterations,
    })
}

#[cfg(all(test, feature = "sensitivity"))]
mod tests {
    use super::*;
    use crate::model::parse_case3;
    use crate::problem::dc_opf_cancellable;
    use crate::sens::{sensitivity, weighted_sensitivity, DcKkt};
    use crate::{DcDerivatives, DcSolver, ExecutionOptions, Mode, Operand, Parameter, Power};

    fn close(actual: &[f64], expected: &[f64], tolerance: f64) {
        let error = actual
            .iter()
            .zip(expected)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        let norm = expected.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(
            error < tolerance * norm.max(1.0),
            "error {error}, norm {norm}, actual {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn dc_backends_match_solutions_columns_and_weighted_gradients() {
        let mut baseline = parse_case3();
        baseline.allow_shed = false;
        baseline.fmax[0] = 0.36;
        let reference = dc_opf_cancellable(&baseline, None).unwrap();
        for solver in [DcSolver::Clarabel, DcSolver::Moreau] {
            for derivatives in [DcDerivatives::Tellegen, DcDerivatives::MoreauSelected] {
                let mut dc = baseline.clone();
                dc.execution = ExecutionOptions {
                    dc_solver: solver,
                    dc_derivatives: derivatives,
                };
                let sol = dc_opf_cancellable(&dc, None).unwrap();
                close(&sol.pg, &reference.pg, 1e-6);
                close(&sol.nu_bal, &reference.nu_bal, 1e-6);
                let system = DcKkt::new(&dc, &sol);
                for parameter in [Parameter::Demand(Power::Active), Parameter::LineLimit] {
                    for operand in [
                        Operand::Price(Power::Active),
                        Operand::Dispatch(Power::Active),
                        Operand::Flow {
                            power: Power::Active,
                            end: crate::End::From,
                        },
                        Operand::Voltage(crate::VoltageKind::Angle),
                    ] {
                        let expected = sensitivity(
                            &DcKkt::new(&baseline, &reference),
                            operand,
                            parameter,
                            Some(&[0]),
                            Mode::Forward,
                        )
                        .unwrap();
                        for mode in [Mode::Forward, Mode::Adjoint, Mode::Auto] {
                            let actual =
                                sensitivity(&system, operand, parameter, Some(&[0]), mode).unwrap();
                            close(
                                &actual.values.iter().flatten().copied().collect::<Vec<_>>(),
                                &expected
                                    .values
                                    .iter()
                                    .flatten()
                                    .copied()
                                    .collect::<Vec<_>>(),
                                1e-3,
                            );
                        }
                    }
                    let weights = [(0, 0.5), (2, 1.0)];
                    let actual = weighted_sensitivity(
                        &system,
                        Operand::Price(Power::Active),
                        &weights,
                        parameter,
                        Some(&[0]),
                    )
                    .unwrap();
                    let expected = weighted_sensitivity(
                        &DcKkt::new(&baseline, &reference),
                        Operand::Price(Power::Active),
                        &weights,
                        parameter,
                        Some(&[0]),
                    )
                    .unwrap();
                    close(&actual, &expected, 1e-3);
                }
            }
        }
    }

    #[test]
    fn moreau_honors_cancellation() {
        let mut dc = parse_case3();
        dc.execution.dc_solver = DcSolver::Moreau;
        let result = dc_opf_cancellable(&dc, Some(Arc::new(AtomicBool::new(true))));
        assert!(result.err().unwrap().contains("cancelled"));
    }
}

#[cfg(feature = "sensitivity")]
pub(crate) fn reduced_matrix(
    value: &clarabel::algebra::CscMatrix<f64>,
    rows: &[Option<usize>],
    columns: &[Option<usize>],
) -> CscMatrix<f64> {
    let mut ri = Vec::new();
    let mut ci = Vec::new();
    let mut values = Vec::new();
    for (col, mapped_col) in columns.iter().enumerate() {
        if let Some(mapped_col) = mapped_col {
            for pos in value.colptr[col]..value.colptr[col + 1] {
                if let Some(row) = rows[value.rowval[pos]] {
                    ri.push(row);
                    ci.push(*mapped_col);
                    values.push(value.nzval[pos]);
                }
            }
        }
    }
    CscMatrix::new_from_triplets(
        rows.iter().flatten().count(),
        columns.iter().flatten().count(),
        ri,
        ci,
        values,
    )
}

#[cfg(all(test, feature = "sensitivity"))]
mod integration_tests {
    use crate::model::{parse_case3, parse_matpower, DcNetwork, CASE3};
    use crate::problem::dc_opf_cancellable;
    use crate::sens::{sensitivity, DcKkt};
    use crate::{DcDerivatives, DcSolver, ExecutionOptions, Mode, Operand, Parameter, Power};

    fn options() -> ExecutionOptions {
        ExecutionOptions {
            dc_solver: DcSolver::Moreau,
            dc_derivatives: DcDerivatives::MoreauSelected,
        }
    }

    #[test]
    fn selected_derivatives_match_finite_differences_across_cost_and_shedding_regimes() {
        let mut congested = parse_case3();
        congested.allow_shed = false;
        congested.fmax[0] = 0.36;
        let mut shedding = parse_case3();
        shedding.gmax = vec![0.4, 0.4];
        shedding.allow_shed = true;
        let piecewise = CASE3
            .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 0 50 5 250 405;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 270 135;");
        let piecewise = DcNetwork::from_network(&parse_matpower(&piecewise).unwrap()).unwrap();
        for mut dc in [congested, shedding, piecewise] {
            dc.execution = options();
            let sol = dc_opf_cancellable(&dc, None).unwrap();
            for (parameter, column, step) in [
                (Parameter::Demand(Power::Active), 1, 1e-2),
                (Parameter::LineLimit, 0, 1e-3),
            ] {
                let analytic = sensitivity(
                    &DcKkt::new(&dc, &sol),
                    Operand::Dispatch(Power::Active),
                    parameter,
                    Some(&[column]),
                    Mode::Forward,
                )
                .unwrap();
                assert_eq!(analytic.implementation.as_deref(), Some("moreau"));
                let mut endpoints = Vec::new();
                for sign in [-1.0, 1.0] {
                    let mut perturbed = dc.clone();
                    if parameter == Parameter::LineLimit {
                        perturbed.fmax[column] += sign * step;
                    } else {
                        perturbed.demand[column] += sign * step;
                    }
                    endpoints.push(dc_opf_cancellable(&perturbed, None).unwrap().pg);
                }
                for (i, row) in analytic.values.iter().enumerate() {
                    let fd = (endpoints[1][i] - endpoints[0][i]) / (2.0 * step);
                    assert!(
                        (row[0] - fd).abs() < 1e-3 * (1.0 + fd.abs()),
                        "{parameter:?}: {} vs {fd}",
                        row[0]
                    );
                }
            }
        }
    }

    #[test]
    fn study_options_survive_forks_commits_and_do_not_change_saved_cases() {
        let net = parse_matpower(CASE3).unwrap();
        let module = powerio::PioModule::new(powerio::PioValue::BalancedNetwork(net));
        let json = crate::ir::serialize_module(&module).unwrap();
        let default = crate::Study::new(&json, crate::Problem::DcOpf).unwrap();
        let mut selected =
            crate::Study::new_with_execution(&json, crate::Problem::DcOpf, options()).unwrap();
        assert_eq!(default.execution(), ExecutionOptions::default());
        assert_eq!(selected.fork().execution(), options());
        assert_eq!(
            selected.save_module().unwrap(),
            default.save_module().unwrap()
        );
        assert_eq!(
            selected.save_instance_module().unwrap(),
            default.save_instance_module().unwrap()
        );
        selected.commit(&[]).unwrap();
        assert_eq!(selected.execution(), options());
        let cells = selected.preview(
            &[crate::NetworkEdit::AddLoad {
                bus: crate::ElementKey::Id(2),
                p_mw: 0.1,
            }],
            &[Operand::Price(Power::Active)],
        );
        assert!(cells.is_ok(), "{cells:?}");
        let response = crate::solve_module_json_with_execution(&json, r#"{"sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}]}"#, &options()).unwrap();
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["sensitivities"][0]["implementation"], "moreau");
        assert_eq!(
            response["sensitivities"][0]["units"],
            "(objective_unit/MW)/MW"
        );
    }

    #[test]
    fn unsupported_selected_parameters_keep_the_specialized_derivative() {
        let mut dc = parse_case3();
        dc.execution = options();
        let sol = dc_opf_cancellable(&dc, None).unwrap();
        let cell = sensitivity(
            &DcKkt::new(&dc, &sol),
            Operand::Dispatch(Power::Active),
            Parameter::Cost(crate::CostTerm::Linear),
            Some(&[0]),
            Mode::Forward,
        )
        .unwrap();
        assert_eq!(cell.implementation.as_deref(), Some("tellegen"));
    }
}

#[cfg(all(test, feature = "sensitivity"))]
mod measurements {
    use crate::model::{parse_matpower, DcNetwork, CASE3};
    use crate::problem::dc_opf_cancellable;
    use crate::sens::{sensitivity, weighted_sensitivity, DcKkt};
    use crate::{DcDerivatives, DcSolver, ExecutionOptions, Mode, Operand, Parameter, Power};
    use std::time::Instant;

    fn measure(mut operation: impl FnMut() -> Result<(), String>) -> serde_json::Value {
        let start = Instant::now();
        if let Err(error) = operation() {
            return serde_json::json!({"error": error});
        }
        let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
        for _ in 0..3 {
            if let Err(error) = operation() {
                return serde_json::json!({"error": error});
            }
        }
        let mut samples = Vec::new();
        for _ in 0..10 {
            let start = Instant::now();
            if let Err(error) = operation() {
                return serde_json::json!({"error": error});
            }
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        serde_json::json!({"cold_ms": cold_ms, "median_ms": (samples[4] + samples[5]) / 2.0, "samples_ms": samples})
    }

    #[test]
    #[ignore = "records performance samples for an explicitly selected case"]
    fn compare_dc_backends() {
        let case = std::env::var("TELLEGEN_COMPARE_CASE").ok();
        let source = case
            .as_ref()
            .map(|path| std::fs::read_to_string(path).unwrap())
            .unwrap_or_else(|| CASE3.to_owned());
        let network = parse_matpower(&source).unwrap();
        let mut baseline = DcNetwork::from_network(&network).unwrap();
        baseline.allow_shed = false;
        if case.is_none() {
            baseline.fmax[0] = 0.36;
        }
        let mut records = Vec::new();
        for solver in [DcSolver::Clarabel, DcSolver::Moreau] {
            for derivatives in [DcDerivatives::Tellegen, DcDerivatives::MoreauSelected] {
                let mut dc = baseline.clone();
                dc.execution = ExecutionOptions {
                    dc_solver: solver,
                    dc_derivatives: derivatives,
                };
                let solve = measure(|| dc_opf_cancellable(&dc, None).map(|_| ()));
                let sol = match dc_opf_cancellable(&dc, None) {
                    Ok(sol) => sol,
                    Err(error) => {
                        records.push(serde_json::json!({"execution": dc.execution, "solve": solve, "error": error}));
                        continue;
                    }
                };
                let kkt = DcKkt::new(&dc, &sol);
                let column = measure(|| {
                    sensitivity(
                        &kkt,
                        Operand::Price(Power::Active),
                        Parameter::Demand(Power::Active),
                        Some(&[1]),
                        Mode::Forward,
                    )
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                });
                let adjoint = measure(|| {
                    weighted_sensitivity(
                        &kkt,
                        Operand::Price(Power::Active),
                        &[(1, 1.0)],
                        Parameter::LineLimit,
                        None,
                    )
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                });
                let assembly = measure(|| {
                    std::hint::black_box(crate::problem::build_opf(
                        &crate::formulation::Dc::new(),
                        &dc,
                    ));
                    Ok(())
                });
                let program = crate::problem::build_opf(&crate::formulation::Dc::new(), &dc);
                let conversion = measure(|| {
                    std::hint::black_box((super::matrix(&program.p), super::matrix(&program.a)));
                    Ok(())
                });
                records.push(serde_json::json!({"execution": dc.execution, "buses": dc.n, "variables": program.q.len(), "constraints": program.b.len(), "assembly": assembly, "moreau_conversion": conversion, "solve": solve, "demand_column_with_setup": column, "weighted_rating_adjoint_with_setup": adjoint, "objective": sol.objective}));
            }
        }
        let value = serde_json::json!({"case": case.unwrap_or_else(|| "congested_case3".into()), "profile": if cfg!(debug_assertions) { "debug" } else { "release" }, "records": records});
        let out = std::env::var("TELLEGEN_COMPARE_OUT")
            .expect("TELLEGEN_COMPARE_OUT must name the JSON output");
        std::fs::write(out, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    }
}
