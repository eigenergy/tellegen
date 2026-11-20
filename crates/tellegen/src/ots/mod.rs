//! Optimal transmission switching on a typed PowerIO AC OTS instance.
//! Enumerates status changes in increasing cardinality. All three dispatch
//! models use the same legal topologies and the same switching objective.
mod linear;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use powerio_matrix::{build_ac_opf_preparation, AcOpfPreparation, AnalysisBranchSource};
use powerio_prob::{
    AcOpfInstance, AcOtsInstance, AcOtsSolution, ObjectiveTerm, TerminalCurrentBounds,
};
use serde::Serialize;

use crate::model::{solve_ots_dispatch, validate_supported, AcOpfSolved};
use crate::{EngineError, ErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OtsModel {
    Lacots,
    Dcots,
    Acots,
}

/// Voltages, generation and terminal powers in PowerIO preparation order.
/// Branch identities refer to closed, energized analysis branches, including
/// lowered windings. Open source rows remain visible in Candidate::closed.
#[derive(Clone, Debug, Serialize)]
pub struct Dispatch {
    pub base_mva: f64,
    pub bus_ids: Vec<usize>,
    pub generator_ids: Vec<String>,
    pub branch_ids: Vec<String>,
    pub va_rad: Vec<f64>,
    pub vm_pu: Option<Vec<f64>>,
    pub pg_pu: Vec<f64>,
    pub qg_pu: Option<Vec<f64>>,
    pub p_from_pu: Vec<f64>,
    pub p_to_pu: Vec<f64>,
    pub q_from_pu: Option<Vec<f64>>,
    pub q_to_pu: Option<Vec<f64>>,
}

impl Dispatch {
    fn new(p: &AcOpfPreparation, va_rad: Vec<f64>, pg_pu: Vec<f64>) -> Self {
        Self {
            base_mva: p.base_mva,
            bus_ids: p.bus_ids.iter().map(|id| id.0).collect(),
            generator_ids: p.generators.identities.clone(),
            branch_ids: p.branches.identities.clone(),
            va_rad,
            pg_pu,
            vm_pu: None,
            qg_pu: None,
            p_from_pu: vec![],
            p_to_pu: vec![],
            q_from_pu: None,
            q_to_pu: None,
        }
    }

    fn from_ac(s: &AcOpfSolved) -> Self {
        let mut d = Self::new(&s.preparation, s.va.clone(), s.pg.clone());
        d.vm_pu = Some(s.vm.clone());
        d.qg_pu = Some(s.qg.clone());
        d.p_from_pu.clone_from(&s.p_from);
        d.p_to_pu.clone_from(&s.p_to);
        d.q_from_pu = Some(s.q_from.clone());
        d.q_to_pu = Some(s.q_to.clone());
        d
    }
}

/// The selected dispatch and its objective, including applicable transition costs.
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub closed: Vec<bool>,
    pub objective: f64,
    pub dispatch: Dispatch,
    /// Present only for ACOTS. Linear dispatches are never emitted as AC solutions.
    #[serde(skip)]
    pub ac_solution: Option<AcOtsSolution>,
}

#[derive(Debug, Serialize)]
pub struct OtsSearch {
    pub model: OtsModel,
    pub best: Option<Candidate>,
    /// Dispatch objective of the initial topology when allowed and solved.
    pub initial_objective: Option<f64>,
    /// All status vectors within the switching budget were visited.
    pub exhausted: bool,
    pub visited: usize,
    pub policy_rejected: usize,
    pub solved: usize,
    /// Certified infeasible convex dispatch subproblems.
    pub infeasible: usize,
    /// Includes local AC failures and unsupported or unresolved subproblems.
    pub failed: usize,
    pub first_failure: Option<String>,
}

pub(crate) fn current_bounds(
    ots: &AcOtsInstance,
    p: &AcOpfPreparation,
    e: usize,
) -> TerminalCurrentBounds {
    match p.branches.analysis_sources[e] {
        AnalysisBranchSource::Branch { row } => ots.branches()[row].current,
        _ => TerminalCurrentBounds::default(),
    }
}

fn check_fixed(ots: &AcOtsInstance, fixed: &AcOpfInstance) -> Result<(), String> {
    validate_supported(fixed)?;
    // PowerIO folds a self-loop into a bus shunt. Such a row has no terminal
    // flow column against which an OTS terminal rating could be checked.
    for (branch, choice) in fixed.network().branches().iter().zip(ots.branches()) {
        if branch.in_service
            && branch.from == branch.to
            && ((branch.rate_a.is_finite()
                && fixed.constraints().thermal_limits.selects(&choice.id))
                || choice.current.from_pu.is_some()
                || choice.current.to_pu.is_some())
        {
            return Err(format!(
                "terminal limits on folded self-loop {} are unsupported",
                choice.id
            ));
        }
    }
    Ok(())
}

fn ac_dispatch(
    ots: &Arc<AcOtsInstance>,
    closed: &[bool],
    fixed: &AcOpfInstance,
    seed: Option<&Dispatch>,
) -> Result<(AcOtsSolution, Dispatch), String> {
    let s = solve_ots_dispatch(ots, fixed, seed)?;
    let d = Dispatch::from_ac(&s);
    let ac = crate::emit::emit_ac_opf_solution(Arc::new(fixed.clone()), &s, "Tellegen OTS")?;
    let solution =
        AcOtsSolution::new(Arc::clone(ots), closed.to_vec(), ac).map_err(|e| e.to_string())?;
    Ok((solution, d))
}

fn dispatch(
    ots: &Arc<AcOtsInstance>,
    closed: &[bool],
    fixed: &AcOpfInstance,
    model: OtsModel,
    switching_cost: f64,
) -> Result<Candidate, EngineError> {
    let unsupported = |e| EngineError::new(ErrorCode::Unsupported, e);
    check_fixed(ots, fixed).map_err(unsupported)?;
    let (objective, dispatch, ac_solution) = if model == OtsModel::Acots {
        let (s, d) = ac_dispatch(ots, closed, fixed, None)
            .map_err(|e| EngineError::new(ErrorCode::NotConverged, e))?;
        (s.objective(), d, Some(s))
    } else {
        let p = build_ac_opf_preparation(fixed, &Default::default())
            .map_err(|e| unsupported(e.to_string()))?;
        let (cost, d) = match model {
            OtsModel::Lacots => linear::lac(ots, &p)?,
            _ => linear::dc(ots, fixed, &p)?,
        };
        let cost = if ots
            .objective()
            .contains(&ObjectiveTerm::BranchSwitchingCost)
        {
            cost + switching_cost
        } else {
            cost
        };
        (cost, d, None)
    };
    Ok(Candidate {
        closed: closed.to_vec(),
        objective,
        dispatch,
        ac_solution,
    })
}

/// Search up to `topology_limit` status vectors, including policy rejections.
/// A budget of one status change takes at most 1 + k dispatches for k candidates.
/// An exhaustive AC run still uses a local nonlinear solver on each topology.
pub fn solve_ots(
    ots: Arc<AcOtsInstance>,
    model: OtsModel,
    topology_limit: usize,
) -> Result<OtsSearch, String> {
    if topology_limit == 0 {
        return Err("topology_limit must be positive".into());
    }
    let rows: Vec<_> = ots
        .branches()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.switchable)
        .map(|(i, _)| i)
        .collect();
    let budget = ots
        .options()
        .max_switched
        .unwrap_or(rows.len())
        .min(rows.len());
    let initial = ots.initial_closed();
    let mut result = OtsSearch {
        model,
        exhausted: true,
        visited: 0,
        policy_rejected: 0,
        solved: 0,
        infeasible: 0,
        failed: 0,
        first_failure: None,
        best: None,
        initial_objective: None,
    };
    for changes in 0..=budget {
        let mut combination: Vec<_> = (0..changes).collect();
        loop {
            if result.visited == topology_limit {
                result.exhausted = false;
                return Ok(result);
            }
            let mut closed = initial.clone();
            for &i in &combination {
                closed[rows[i]] = !closed[rows[i]];
            }
            result.visited += 1;
            match ots.validate_topology(&closed) {
                Err(_) => result.policy_rejected += 1,
                Ok(cost) => {
                    let (fixed, _) = ots.fix_topology(&closed).map_err(|e| e.to_string())?;
                    match dispatch(&ots, &closed, &fixed, model, cost) {
                        Ok(c) => {
                            result.solved += 1;
                            if changes == 0 {
                                result.initial_objective = Some(c.objective);
                            }
                            // Keep the lower-cardinality incumbent in numerical ties.
                            let better = result.best.as_ref().is_none_or(|b| {
                                c.objective < b.objective - 1e-8 * b.objective.abs().max(1.0)
                            });
                            if better {
                                result.best = Some(c);
                            }
                        }
                        Err(e) if e.code == ErrorCode::Infeasible => result.infeasible += 1,
                        Err(e) => {
                            result.failed += 1;
                            result.first_failure.get_or_insert_with(|| {
                                format!(
                                    "changed source rows {:?}: {e}",
                                    combination.iter().map(|&i| rows[i]).collect::<Vec<_>>()
                                )
                            });
                        }
                    }
                }
            }
            let Some(j) = (0..changes)
                .rev()
                .find(|&j| combination[j] < rows.len() - changes + j)
            else {
                break;
            };
            combination[j] += 1;
            for i in j + 1..changes {
                combination[i] = combination[i - 1] + 1;
            }
        }
    }
    Ok(result)
}

#[derive(Debug)]
pub struct AcRestoration {
    pub solution: AcOtsSolution,
    pub dispatch: Dispatch,
    /// Maximum bus mismatches when the surrogate dispatch is replayed under
    /// nonlinear AC physics. DC replay uses unit voltage and scheduled Qg.
    pub initial_p_mismatch_mw: f64,
    pub initial_q_mismatch_mvar: f64,
    pub redispatch_l1_mw: f64,
    /// If the warm solve failed, this records its error after a successful retry.
    pub warm_start_error: Option<String>,
}

/// Fix the candidate topology, replay its dispatch, and redispatch generation
/// under the full AC equations and selected equipment limits. No load is shed.
pub fn restore_ac(ots: Arc<AcOtsInstance>, plan: &Candidate) -> Result<AcRestoration, String> {
    let (fixed, _) = ots.fix_topology(&plan.closed).map_err(|e| e.to_string())?;
    check_fixed(&ots, &fixed)?;
    let p = build_ac_opf_preparation(&fixed, &Default::default()).map_err(|e| e.to_string())?;
    let d = &plan.dispatch;
    let n = p.n_buses;
    let k = p.n_generators();
    if d.base_mva != p.base_mva
        || !p
            .bus_ids
            .iter()
            .map(|id| id.0)
            .eq(d.bus_ids.iter().copied())
        || d.generator_ids != p.generators.identities
        || d.branch_ids != p.branches.identities
        || d.va_rad.len() != n
        || d.pg_pu.len() != k
        || d.vm_pu.as_ref().is_some_and(|v| v.len() != n)
        || d.qg_pu.as_ref().is_some_and(|v| v.len() != k)
        || d.va_rad
            .iter()
            .chain(&d.pg_pu)
            .chain(d.vm_pu.iter().flatten())
            .chain(d.qg_pu.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err(
            "candidate dispatch does not match the fixed topology or contains nonfinite values"
                .into(),
        );
    }
    let flat = vec![1.0; n];
    let vm = d.vm_pu.as_deref().unwrap_or(&flat);
    let qg = d.qg_pu.as_deref().unwrap_or(&p.generators.qg);
    let (inj_p, inj_q) =
        powerio_matrix::calc_bus_power_injection(&p.calc_admittance_matrix(), vm, &d.va_rad)
            .map_err(|e| e.to_string())?;
    let mismatch = |generation: &[f64], demand: &[f64], injection: &[f64]| {
        powerio_matrix::calc_bus_totals(n, &p.generators.bus_of_gen, generation)
            .iter()
            .zip(demand)
            .zip(injection)
            .map(|((&g, &l), &s)| (g - l - s).abs() * p.base_mva)
            .fold(0.0, f64::max)
    };
    let initial_p_mismatch_mw = mismatch(&d.pg_pu, &p.buses.p_d, &inj_p);
    let initial_q_mismatch_mvar = mismatch(qg, &p.buses.q_d, &inj_q);
    let ((solution, restored), warm_start_error) =
        match ac_dispatch(&ots, &plan.closed, &fixed, Some(d)) {
            Ok(s) => (s, None),
            Err(warm) => (
                ac_dispatch(&ots, &plan.closed, &fixed, None).map_err(|cold| {
                    format!("AC restoration failed from candidate ({warm}) and case ({cold})")
                })?,
                Some(warm),
            ),
        };
    let redispatch_l1_mw = restored
        .pg_pu
        .iter()
        .zip(&d.pg_pu)
        .map(|(a, b)| (a - b).abs() * p.base_mva)
        .sum();
    Ok(AcRestoration {
        solution,
        dispatch: restored,
        initial_p_mismatch_mw,
        initial_q_mismatch_mvar,
        redispatch_l1_mw,
        warm_start_error,
    })
}
