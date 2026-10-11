//! Experimental native multiconductor IVR OPF.
//!
//! The `mc-opf` feature is opt-in. PowerIO owns preparation and identities;
//! POUNCE evaluates exact sparse derivatives. Accepted points are independently
//! checked against the physical equations. Success is local, with no prices,
//! optimality certificate, retained session, or solution sensitivities.
mod model;
#[cfg(test)]
mod tests;
mod validate;

use crate::nlp::CancellableNlTnlp;
use pounce_nl::nl_reader::NlTnlp;
use pounce_rs::{ApplicationReturnStatus, IpoptApplication, TNLP};
use powerio::{McAcOpfInstance, McAcOpfSolution, PioModule, PioValue, Termination};
use powerio_matrix::{build_mc_ac_opf_preparation, McAcOpfAssemblyOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    pub identity: String,
    pub current_from_a: Vec<[f64; 2]>,
    pub current_to_a: Vec<[f64; 2]>,
    pub power_from_va: Vec<[f64; 2]>,
    pub power_to_va: Vec<[f64; 2]>,
}
/// Coil powers and currents are distinct from terminal powers, especially for delta.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfDeviceResult {
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
    pub identity: String,
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
    pub solution: McAcOpfSolution,
    pub residuals: McOpfResiduals,
    pub branches: Vec<McOpfBranchResult>,
    pub devices: Vec<McOpfDeviceResult>,
    pub transformers: Vec<McOpfTransformerResult>,
    pub iterations: i32,
    pub fingerprint: String,
    pub profile: Option<McOpfProfile>,
}
fn cancelled(cancel: Option<&AtomicBool>) -> Result<(), String> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        Err("multiconductor OPF cancelled".into())
    } else {
        Ok(())
    }
}
/// Solve and validate a typed fixed-equipment multiconductor OPF instance.
/// Source data are immutable. No result is returned on an unaccepted NLP status.
pub fn solve_mc_ac_opf_instance(
    instance: Arc<McAcOpfInstance>,
    options: &McOpfOptions,
) -> Result<McOpfResult, String> {
    solve_mc_ac_opf_instance_cancellable(instance, options, None)
}
/// As [`solve_mc_ac_opf_instance`], with cancellation before/after compilation
/// and at POUNCE iteration boundaries. Browser hard termination is separate.
pub fn solve_mc_ac_opf_instance_cancellable(
    instance: Arc<McAcOpfInstance>,
    options: &McOpfOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<McOpfResult, String> {
    cancelled(cancel.as_deref())?;
    if !options.acceptance_tolerance.is_finite()
        || options.acceptance_tolerance <= 0.0
        || !options.objective_scale.is_finite()
        || options.objective_scale <= 0.0
        || options.max_iterations == 0
        || options.max_iterations > i32::MAX as u32
    {
        return Err("invalid multiconductor OPF options".into());
    }
    let mut profile = options.collect_profile.then(McOpfProfile::default);
    let mut stage = Instant::now();
    let prep = build_mc_ac_opf_preparation(
        &instance,
        &McAcOpfAssemblyOptions::new(options.voltage_base_v, options.power_base_va),
    )
    .map_err(|e| e.to_string())?;
    record_stage(&mut profile, &mut stage, "preparation");
    let fingerprint = Sha256::digest(
        [
            b"tellegen/mc-ivr-v2\0".as_slice(),
            &serde_json::to_vec(&prep).map_err(|e| e.to_string())?,
        ]
        .concat(),
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect();
    record_stage(&mut profile, &mut stage, "fingerprint");
    let model = model::compile(prep)?;
    record_stage(&mut profile, &mut stage, "model_compile");
    cancelled(cancel.as_deref())?;
    let tnlp = Rc::new(RefCell::new(CancellableNlTnlp::new(
        NlTnlp::try_new(model.problem.clone())?,
        cancel.clone(),
    )));
    record_stage(&mut profile, &mut stage, "evaluator_setup");
    let mut app = IpoptApplication::new();
    app.initialize_with_options_str(&format!("linear_solver feral\nhessian_approximation exact\nnlp_scaling_method none\nlinear_system_scaling none\nbound_relax_factor 0\ntol 1e-9\nconstr_viol_tol 1e-9\nacceptable_tol 1e-8\nmax_iter {}\nobj_scaling_factor {}\nprint_level 0\n",options.max_iterations,options.objective_scale)).map_err(|e|e.to_string())?;
    if options.collect_profile {
        app.initialize_with_options_str("timing_statistics yes\n")
            .map_err(|e| e.to_string())?;
    }
    app.initialize().map_err(|e| e.to_string())?;
    record_stage(&mut profile, &mut stage, "solver_setup");
    let status = app.optimize_tnlp(Rc::clone(&tnlp) as Rc<RefCell<dyn TNLP>>);
    record_stage(&mut profile, &mut stage, "solve");
    cancelled(cancel.as_deref())?;
    if !matches!(
        status,
        ApplicationReturnStatus::SolveSucceeded | ApplicationReturnStatus::SolvedToAcceptableLevel
    ) {
        return Err(format!("multiconductor OPF stopped with {status:?}; a local NLP failure is not an infeasibility proof"));
    }
    let t = tnlp.borrow();
    let x = t.inner.final_x().ok_or("solver returned no primal point")?;
    let checked = validate::check(&model, x, t.inner.final_obj(), options.acceptance_tolerance)?;
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
    .map_err(|e| e.to_string())?
    .with_producer(format!(
        "tellegen {} mc-ivr-v1 POUNCE local solution",
        crate::VERSION
    ));
    record_stage(&mut profile, &mut stage, "solution_projection");
    if let Some(p) = &mut profile {
        collect_solver_profile(p, &app);
    }
    Ok(McOpfResult {
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
pub fn solve_mc_ac_opf_module_json(input: &str, options: &McOpfOptions) -> Result<String, String> {
    let module = crate::ir::deserialize_module(input)?;
    for diagnostic in module.diagnostics() {
        if diagnostic.severity() == powerio::DiagnosticSeverity::Error
            || [
                "UNSUPPORTED",
                "MALFORMED",
                "FIELD_DROPPED",
                "RECORD_DROPPED",
            ]
            .iter()
            .any(|code| diagnostic.code().contains(code))
        {
            return Err(format!(
                "MC OPF input has a lossy or unsupported diagnostic: {}: {}",
                diagnostic.code(),
                diagnostic.message()
            ));
        }
    }
    let PioValue::McAcOpfInstance(instance) = module.into_value() else {
        return Err("expected a PowerIO McAcOpfInstance module".into());
    };
    let result = solve_mc_ac_opf_instance(Arc::new(instance), options)?;
    crate::ir::serialize_module(&PioModule::new(PioValue::McAcOpfSolution(result.solution)))
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
