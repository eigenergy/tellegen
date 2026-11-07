//! Experimental native multiconductor IVR OPF.
//!
//! The `mc-opf` feature is opt-in. PowerIO owns preparation and identities;
//! POUNCE evaluates exact sparse derivatives. Accepted points are independently
//! checked against the physical equations. Success is local, with no prices,
//! optimality certificate, retained session, or solution sensitivities.
mod diagnostics;
mod model;
mod result;
pub use diagnostics::{McOpfDiagnostic, McOpfError, McOpfErrorKind, McOpfStage, McOpfViolation};
pub use result::{
    McOpfCoilAxis, McOpfPortResult, McOpfShuntResult, McOpfTerminalId, McOpfTerminalVoltage,
    MC_OPF_RESULT_EXTENSION,
};
#[cfg(test)]
mod tests;
mod validate;

use crate::nlp::{configure_restoration, CancellableNlTnlp};
use pounce_nl::nl_reader::NlTnlp;
use pounce_rs::{ApplicationReturnStatus, IpoptApplication, TNLP};
use powerio::{McAcOpfInstance, McAcOpfSolution, PioModule, PioValue, Termination};
use powerio_matrix::{build_mc_ac_opf_preparation, McAcOpfAssemblyOptions};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

/// Numerical choices, separate from the declared PowerIO problem.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McOpfOptions {
    pub voltage_base_v: f64,
    pub power_base_va: f64,
    /// Independent maximum normalized residual accepted after a solve.
    pub acceptance_tolerance: f64,
    pub max_iterations: u32,
    /// Positive numerical objective scaling; reported costs retain physical units.
    pub objective_scale: f64,
    /// Collect stage and detailed solver timings. Off by default: timers add overhead.
    pub collect_profile: bool,
}
impl Default for McOpfOptions {
    fn default() -> Self {
        Self {
            voltage_base_v: 230.0,
            power_base_va: 1000.0,
            acceptance_tolerance: 1e-6,
            max_iterations: 1000,
            objective_scale: 1.0,
            collect_profile: false,
        }
    }
}
impl McOpfOptions {
    fn validate(&self) -> Result<(), McOpfError> {
        let mut invalid: Vec<_> = [
            ("voltage_base_v", self.voltage_base_v),
            ("power_base_va", self.power_base_va),
            ("acceptance_tolerance", self.acceptance_tolerance),
            ("objective_scale", self.objective_scale),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_finite() || *v <= 0.0)
        .map(|(name, _)| (name, "must be finite and positive"))
        .collect();
        if self.max_iterations == 0 || self.max_iterations > i32::MAX as u32 {
            invalid.push(("max_iterations", "must be in 1..=i32::MAX"));
        }
        if invalid.is_empty() {
            return Ok(());
        }
        let mut error = McOpfError::new(
            McOpfErrorKind::InvalidInput,
            McOpfStage::Input,
            "invalid multiconductor OPF options",
        );
        error.diagnostics = invalid
            .into_iter()
            .map(|(name, requirement)| McOpfDiagnostic {
                code: "MC_OPF.INVALID_OPTION".into(),
                severity: powerio::DiagnosticSeverity::Error,
                message: format!("{name} {requirement}"),
                target: Some(format!("/options/{name}")),
            })
            .collect();
        Err(error)
    }
}
/// Independently recomputed residuals, in the explicitly selected working units.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct McOpfResiduals {
    pub kcl_pu: f64,
    pub kvl_pu: f64,
    pub power_link_pu: f64,
    pub prescribed_power_pu: f64,
    pub relative_limit_violation: f64,
    pub objective_relative_error: f64,
}
impl McOpfResiduals {
    pub fn maximum(&self) -> f64 {
        [
            self.kcl_pu,
            self.kvl_pu,
            self.power_link_pu,
            self.prescribed_power_pu,
            self.relative_limit_violation,
            self.objective_relative_error,
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }
}
/// A current and power ledger at one branch end; positive current leaves the bus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfBranchResult {
    pub from_terminals: Vec<McOpfTerminalId>,
    pub to_terminals: Vec<McOpfTerminalId>,
    pub open: bool,
    pub identity: String,
    pub current_from_a: Vec<[f64; 2]>,
    pub current_to_a: Vec<[f64; 2]>,
    pub power_from_va: Vec<[f64; 2]>,
    pub power_to_va: Vec<[f64; 2]>,
}
/// Coil powers and currents are distinct from terminal powers, especially for delta.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfDeviceResult {
    pub terminals: Vec<McOpfTerminalId>,
    pub coils: Vec<McOpfCoilAxis>,
    /// Same injection/withdrawal sign convention as terminal_power_va.
    pub terminal_current_a: Vec<[f64; 2]>,
    pub identity: String,
    pub kind: powerio_matrix::McOpfDeviceKind,
    pub coil_power_va: Vec<[f64; 2]>,
    pub coil_current_a: Vec<[f64; 2]>,
    /// In original terminal-map order; positive is injection for sources/generators
    /// and withdrawal for loads. Includes the neutral return contribution.
    pub terminal_power_va: Vec<[f64; 2]>,
}
/// Bare winding coil quantities, in winding then coil order. Core/grounding
/// shunts are separate electrical stamps and are excluded from these currents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfTransformerResult {
    pub coils: Vec<McOpfCoilAxis>,
    /// Prepared rating ports; these can include magnetizing current and have
    /// different axes from bare winding coils. They are not additive terminals.
    pub ports: Vec<McOpfPortResult>,
    pub identity: String,
    /// Optimized physical tap coefficient, in the canonical winding convention.
    /// None means fixed taps, retained in solution.instance().network().
    pub tap: Option<f64>,
    pub coil_current_a: Vec<[f64; 2]>,
    pub coil_power_va: Vec<[f64; 2]>,
}
/// Opt-in wall-clock measurements. Solver subsystem timers overlap the stage
/// timers and each other; they must not be summed as a disjoint breakdown.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct McOpfProfile {
    pub stages_s: BTreeMap<String, f64>,
    pub solver_s: BTreeMap<String, f64>,
    pub evaluations: BTreeMap<String, i32>,
    pub linear_solver: Option<McOpfLinearProfile>,
    pub restoration_calls: i32,
    pub quality_escalations: i32,
    pub final_unscaled_dual_inf: f64,
    pub final_unscaled_complementarity: f64,
}
/// Backend counters distinguish numeric factor work from regularization retries
/// and symbolic pattern changes. Missing values mean the backend did not report them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfLinearProfile {
    pub factors: u64,
    pub pattern_reuses: u64,
    pub pattern_changes: u64,
    pub numeric_factor_s: f64,
    pub delayed_columns: u64,
    pub last_matrix_nnz: Option<usize>,
    pub last_factor_nnz: Option<usize>,
    pub max_fill_ratio: Option<f64>,
    pub last_ordering: Option<String>,
}
#[derive(Clone, Debug)]
pub struct McOpfResult {
    // Retain provenance and unrelated extensions when a saved result is read.
    pub(super) module: Option<PioModule<PioValue>>,
    pub terminals: Vec<McOpfTerminalVoltage>,
    pub shunts: Vec<McOpfShuntResult>,
    pub options: McOpfOptions,
    pub diagnostics: Vec<McOpfDiagnostic>,
    pub solution: McAcOpfSolution,
    pub residuals: McOpfResiduals,
    pub branches: Vec<McOpfBranchResult>,
    pub devices: Vec<McOpfDeviceResult>,
    pub transformers: Vec<McOpfTransformerResult>,
    pub iterations: i32,
    pub fingerprint: String,
    pub profile: Option<McOpfProfile>,
}
fn cancelled(cancel: Option<&AtomicBool>, stage: McOpfStage) -> Result<(), McOpfError> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err(McOpfError::new(
            McOpfErrorKind::Cancelled,
            stage,
            "multiconductor OPF cancelled",
        ))
    } else {
        Ok(())
    }
}
/// Solve and validate a typed fixed-equipment multiconductor OPF instance.
/// Source data are immutable. No result is returned on an unaccepted NLP status.
pub fn solve_mc_ac_opf_instance(
    instance: Arc<McAcOpfInstance>,
    options: &McOpfOptions,
) -> Result<McOpfResult, McOpfError> {
    solve_mc_ac_opf_instance_cancellable(instance, options, None)
}
/// As [`solve_mc_ac_opf_instance`], with cancellation before/after compilation
/// and at POUNCE iteration boundaries. Browser hard termination is separate.
pub fn solve_mc_ac_opf_instance_cancellable(
    instance: Arc<McAcOpfInstance>,
    options: &McOpfOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<McOpfResult, McOpfError> {
    cancelled(cancel.as_deref(), McOpfStage::Input)?;
    options.validate()?;
    let mut profile = options.collect_profile.then(McOpfProfile::default);
    let mut stage = Instant::now();
    let prep = build_mc_ac_opf_preparation(
        &instance,
        &McAcOpfAssemblyOptions::new(options.voltage_base_v, options.power_base_va),
    )
    .map_err(|e| McOpfError::from_powerio(McOpfStage::Preparation, e))?;
    record_stage(&mut profile, &mut stage, "preparation");
    let fingerprint = result::fingerprint(&prep)?;
    record_stage(&mut profile, &mut stage, "fingerprint");
    let model = model::compile(prep)
        .map_err(|e| McOpfError::new(McOpfErrorKind::InvalidInput, McOpfStage::Compilation, e))?;
    record_stage(&mut profile, &mut stage, "model_compile");
    cancelled(cancel.as_deref(), McOpfStage::Compilation)?;
    let tnlp = Rc::new(RefCell::new(CancellableNlTnlp::new(
        NlTnlp::try_new(model.problem.clone()).map_err(|e| {
            McOpfError::new(McOpfErrorKind::NumericalFailure, McOpfStage::Compilation, e)
        })?,
        cancel.clone(),
    )));
    record_stage(&mut profile, &mut stage, "evaluator_setup");
    let mut app = IpoptApplication::new();
    app.initialize_with_options_str(&format!("linear_solver feral\nhessian_approximation exact\nnlp_scaling_method none\nlinear_system_scaling none\nbound_relax_factor 0\ntol 1e-9\nconstr_viol_tol 1e-9\nacceptable_tol 1e-8\nmax_iter {}\nobj_scaling_factor {}\nprint_level 0\n",options.max_iterations,options.objective_scale)).map_err(|e| McOpfError::new(McOpfErrorKind::NumericalFailure, McOpfStage::Solver, e.to_string()))?;
    if options.collect_profile {
        app.initialize_with_options_str("timing_statistics yes\n")
            .map_err(|e| {
                McOpfError::new(
                    McOpfErrorKind::NumericalFailure,
                    McOpfStage::Solver,
                    e.to_string(),
                )
            })?;
    }
    configure_restoration(&mut app);
    app.initialize().map_err(|e| {
        McOpfError::new(
            McOpfErrorKind::NumericalFailure,
            McOpfStage::Solver,
            e.to_string(),
        )
    })?;
    record_stage(&mut profile, &mut stage, "solver_setup");
    let status = app.optimize_tnlp(Rc::clone(&tnlp) as Rc<RefCell<dyn TNLP>>);
    record_stage(&mut profile, &mut stage, "solve");
    cancelled(cancel.as_deref(), McOpfStage::Solver)?;
    if !matches!(
        status,
        ApplicationReturnStatus::SolveSucceeded | ApplicationReturnStatus::SolvedToAcceptableLevel
    ) {
        let mut error = McOpfError::new(McOpfErrorKind::NumericalFailure, McOpfStage::Solver, format!("multiconductor OPF stopped with {status:?}; a local NLP failure is not an infeasibility proof"));
        error.solver_status = Some(format!("{status:?}"));
        return Err(error);
    }
    let t = tnlp.borrow();
    let x = t.inner.final_x().ok_or_else(|| {
        McOpfError::new(
            McOpfErrorKind::NumericalFailure,
            McOpfStage::Solver,
            "solver returned no primal point",
        )
    })?;
    let checked = validate::check(&model, x, t.inner.final_obj(), options.acceptance_tolerance)?;
    let pf_diagnostics = validate::check_power_factors(
        instance.network(),
        &checked.devices,
        options.power_base_va,
        options.acceptance_tolerance,
    )?;
    cancelled(cancel.as_deref(), McOpfStage::Validation)?;
    record_stage(&mut profile, &mut stage, "validation");
    let source = checked
        .devices
        .iter()
        .filter(|d| d.kind == powerio_matrix::McOpfDeviceKind::Source)
        .flat_map(|d| d.terminal_power_va.iter().map(|p| p[0]))
        .collect();
    let generators = checked
        .devices
        .iter()
        .filter(|d| d.kind == powerio_matrix::McOpfDeviceKind::Generator)
        .flat_map(|d| d.terminal_power_va.iter().map(|p| p[0]))
        .collect();
    let solution = McAcOpfSolution::new(
        instance,
        Termination::Converged,
        checked
            .voltage
            .iter()
            .map(|v| v.norm() * options.voltage_base_v)
            .collect(),
        checked.voltage.iter().map(|v| v.arg()).collect(),
        source,
        generators,
        checked.objective,
    )
    .map_err(|e| McOpfError::from_powerio(McOpfStage::Export, e))?
    .with_producer(format!(
        "tellegen {} mc-ivr-v2 POUNCE local solution",
        crate::VERSION
    ));
    record_stage(&mut profile, &mut stage, "solution_projection");
    if let Some(p) = &mut profile {
        collect_solver_profile(p, &app);
    }
    Ok(McOpfResult {
        module: None,
        terminals: model
            .prep
            .terminals
            .iter()
            .zip(&checked.voltage)
            .map(|(t, v)| McOpfTerminalVoltage {
                terminal: McOpfTerminalId {
                    bus: t.bus.clone(),
                    terminal: t.terminal.clone(),
                },
                voltage_v: [v.re * options.voltage_base_v, v.im * options.voltage_base_v],
            })
            .collect(),
        shunts: checked.shunts,
        options: options.clone(),
        diagnostics: pf_diagnostics,
        solution,
        residuals: checked.residuals,
        branches: checked.branches,
        transformers: checked.transformers,
        devices: checked.devices,
        iterations: app.statistics().iteration_count,
        fingerprint,
        profile,
    })
}
/// Solve an explicitly declared MC OPF module and emit a canonical solution module.
/// A bare network is refused, so no objective or constraint selection is invented.
pub fn solve_mc_ac_opf_module_json(
    input: &str,
    options: &McOpfOptions,
) -> Result<String, McOpfError> {
    let module = crate::ir::deserialize_module_with_diagnostics(input)
        .map_err(|e| McOpfError::from_powerio(McOpfStage::Input, e))?;
    validate_mc_opf_input_diagnostics(&module)?;
    let PioValue::McAcOpfInstance(instance) = module.value() else {
        return Err(McOpfError::new(
            McOpfErrorKind::InvalidInput,
            McOpfStage::Input,
            "expected a PowerIO McAcOpfInstance module",
        ));
    };
    let mut result =
        solve_mc_ac_opf_instance(Arc::new(instance.clone()), options).map_err(|mut error| {
            let mut findings: Vec<_> = module.diagnostics().iter().map(Into::into).collect();
            findings.append(&mut error.diagnostics);
            error.diagnostics = findings;
            error
        })?;
    result
        .diagnostics
        .extend(module.diagnostics().iter().map(McOpfDiagnostic::from));
    let mut output = module
        .map_value(|_| PioValue::McAcOpfSolution(result.solution.clone()))
        .sever_source();
    output.sever_value_targets();
    result.insert_report(&mut output)?;
    crate::ir::serialize_module(&output)
        .map_err(|e| McOpfError::new(McOpfErrorKind::SerializationFailure, McOpfStage::Export, e))
}

/// Reject conversion errors and dropped or unsupported physical data before
/// extracting a network or instance from its module. Typed-instance callers
/// importing external data must retain and check these module diagnostics:
/// physical validation cannot recover constraints already lost during parsing.
pub fn validate_mc_opf_input_diagnostics<T>(module: &PioModule<T>) -> Result<(), McOpfError> {
    let rejected: Vec<_> = module
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity() == powerio::DiagnosticSeverity::Error
                || [
                    "UNSUPPORTED",
                    "MALFORMED",
                    "FIELD_DROPPED",
                    "RECORD_DROPPED",
                ]
                .iter()
                .any(|code| diagnostic.code().contains(code))
        })
        .collect();
    if let Some(first) = rejected.first() {
        let kind = if rejected.iter().any(|d| d.code().contains("UNSUPPORTED")) {
            McOpfErrorKind::UnsupportedInput
        } else {
            McOpfErrorKind::InvalidInput
        };
        let mut error = McOpfError::new(
            kind,
            McOpfStage::Input,
            format!(
                "MC OPF input has a lossy or unsupported diagnostic: {}: {}",
                first.code(),
                first.message()
            ),
        );
        error.diagnostics = module.diagnostics().iter().map(Into::into).collect();
        return Err(error);
    }
    Ok(())
}

fn record_stage(profile: &mut Option<McOpfProfile>, stage: &mut Instant, name: &str) {
    if let Some(p) = profile {
        let now = Instant::now();
        p.stages_s
            .insert(name.into(), now.duration_since(*stage).as_secs_f64());
        *stage = now;
    }
}
fn collect_solver_profile(profile: &mut McOpfProfile, app: &IpoptApplication) {
    let t = app.timing_stats();
    for (name, timer) in [
        ("overall_algorithm", &t.overall_alg),
        ("initialize_iterates", &t.initialize_iterates),
        ("update_hessian", &t.update_hessian),
        ("barrier_update", &t.update_barrier_parameter),
        ("search_direction", &t.compute_search_direction),
        ("line_search", &t.compute_acceptable_trial_point),
        ("check_convergence", &t.check_convergence),
        (
            "symbolic_factorization",
            &t.linear_system_symbolic_factorization,
        ),
        ("factorization", &t.linear_system_factorization),
        ("back_solve", &t.linear_system_back_solve),
        ("function_evaluations", &t.total_function_evaluation_time),
        ("objective", &t.eval_obj),
        ("objective_gradient", &t.eval_grad_obj),
        ("constraints", &t.eval_constr),
        ("jacobian", &t.eval_constr_jac),
        ("hessian", &t.eval_lag_hess),
    ] {
        profile
            .solver_s
            .insert(name.into(), timer.total_wallclock_time());
    }
    profile.linear_solver = app.linear_solver_summary().map(|s| McOpfLinearProfile {
        factors: s.n_factors,
        pattern_reuses: s.n_pattern_reuse,
        pattern_changes: s.n_pattern_changes,
        numeric_factor_s: s.total_factor_secs,
        delayed_columns: s.total_delayed_cols,
        last_matrix_nnz: s.last_nnz_a,
        last_factor_nnz: s.last_nnz_l,
        max_fill_ratio: s.max_fill_ratio,
        last_ordering: s.last_ordering,
    });
    let s = app.statistics();
    for (name, count) in [
        ("objective", s.num_obj_evals),
        ("constraints", s.num_constr_evals),
        ("objective_gradient", s.num_obj_grad_evals),
        ("jacobian", s.num_constr_jac_evals),
        ("hessian", s.num_hess_evals),
    ] {
        profile.evaluations.insert(name.into(), count);
    }
    profile.restoration_calls = s.restoration_calls;
    profile.quality_escalations = s.quality_escalations;
    profile.final_unscaled_dual_inf = s.final_unscaled_dual_inf;
    profile.final_unscaled_complementarity = s.final_unscaled_compl;
}
