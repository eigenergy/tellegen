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
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
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
}
impl Default for McOpfOptions {
    fn default() -> Self {
        Self {
            voltage_base_v: 230.0,
            power_base_va: 1000.0,
            acceptance_tolerance: 1e-6,
            max_iterations: 1000,
            objective_scale: 1.0,
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
#[derive(Clone, Debug)]
pub struct McOpfResult {
    pub solution: McAcOpfSolution,
    pub residuals: McOpfResiduals,
    pub branches: Vec<McOpfBranchResult>,
    pub devices: Vec<McOpfDeviceResult>,
    pub transformers: Vec<McOpfTransformerResult>,
    pub iterations: i32,
    pub fingerprint: String,
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
    let prep = build_mc_ac_opf_preparation(
        &instance,
        &McAcOpfAssemblyOptions::new(options.voltage_base_v, options.power_base_va),
    )
    .map_err(|e| e.to_string())?;
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
    let model = model::compile(prep)?;
    cancelled(cancel.as_deref())?;
    let tnlp = Rc::new(RefCell::new(CancellableNlTnlp::new(
        NlTnlp::try_new(model.problem.clone())?,
        cancel.clone(),
    )));
    let mut app = IpoptApplication::new();
    app.initialize_with_options_str(&format!("linear_solver feral\nhessian_approximation exact\nnlp_scaling_method none\nlinear_system_scaling none\nbound_relax_factor 0\ntol 1e-9\nconstr_viol_tol 1e-9\nacceptable_tol 1e-8\nmax_iter {}\nobj_scaling_factor {}\nprint_level 0\n",options.max_iterations,options.objective_scale)).map_err(|e|e.to_string())?;
    app.initialize().map_err(|e| e.to_string())?;
    let status = app.optimize_tnlp(Rc::clone(&tnlp) as Rc<RefCell<dyn TNLP>>);
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
    Ok(McOpfResult {
        solution,
        residuals: checked.residuals,
        branches: checked.branches,
        transformers: checked.transformers,
        devices: checked.devices,
        iterations: app.statistics().iteration_count,
        fingerprint,
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
