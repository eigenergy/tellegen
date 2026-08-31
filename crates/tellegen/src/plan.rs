//! Differentiable capacity planning: a bounded search over branch thermal
//! ratings, steered by the implicit gradient of an outer objective through
//! the solved DC OPF KKT system.
//!
//! The inner solve is the exact DC OPF `F(z, c) = 0` (its KKT residuals at
//! the optimum); the outer objective is a scalar over the optimal solution,
//! `Phi(z*(c))`. The gradient `dPhi/dc` comes from one weighted adjoint
//! solve through the internal `weighted_sensitivity` routine
//! — never a dense buses by branches matrix — and every step the search
//! takes is verified by an exact re-solve. The recorded iterations carry the
//! direction, prediction, exact outcome, and first order error.
//!
//! The objective and decision specifications are plain serde data: stable
//! PowerIO identities, weights, and bounds. No executable code crosses this
//! boundary, which is what makes the surface safe to expose to a browser
//! agent or an MCP tool.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::model::DcNetwork;
use crate::problem::{dc_opf_cancellable, DcOpfSolution};
use crate::sens::{weighted_sensitivity, DcKkt, Differentiable, Operand, Parameter, Power};
use powerio::BusId;
use powerio_prob::DcOpfInstance;

/// A safe, serializable outer objective over the optimal solve. Every
/// variant is a scalar with a well defined gradient through the KKT system;
/// new observables (generation, line loading, operating cost) add variants
/// rather than accepting code.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImplicitObjective {
    /// `Phi = Σ_b w_b · lmp_b` over the selected buses. Positive weights
    /// make the search look for capacity increases that lower the weighted
    /// marginal value of demand.
    WeightedLmp { weights: Vec<BusWeight> },
}

/// One bus term of a weighted objective, addressed by PowerIO bus ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BusWeight {
    pub bus: BusId,
    pub weight: f64,
}

/// The bounded decision space and search limits of one planning run. All
/// power quantities are MW.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CapacityPlanSpec {
    pub objective: ImplicitObjective,
    /// Candidate branches whose thermal ratings the search may increase.
    pub candidates: Vec<String>,
    /// Maximum cumulative increase for one branch.
    pub max_increase_per_branch_mw: f64,
    /// Total intervention budget: `Σ_e increase_e` never exceeds it.
    pub budget_mw: f64,
    /// Discrete capacity increase considered in one trial.
    pub increment_mw: f64,
    /// Positive cardinality bound over the final proposal, not one iteration.
    pub max_changed_lines: usize,
    /// Maximum number of exact OPF solves, including the baseline and every
    /// accepted or rejected trial.
    pub exact_solve_budget: usize,
}

/// One branch rating change of a proposal, MW, by canonical PowerIO identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RatingChange {
    pub branch: String,
    pub delta_mw: f64,
}

/// One implicit gradient entry, in outer objective units per MW, by canonical
/// PowerIO identity.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GradientEntry {
    pub branch: String,
    pub value: f64,
}

/// One recorded search step: the direction the gradient named, the rating
/// trial, the first order prediction, the exact outcome, and
/// whether the step was kept.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CapacityPlanIteration {
    /// `dPhi/dc` at the step's starting point for every candidate, in outer
    /// objective units per MW.
    pub gradient: Vec<GradientEntry>,
    /// The trial rating changes, MW.
    pub delta_mw: Vec<RatingChange>,
    /// First order prediction `Σ g_e · delta_e`.
    pub predicted_phi_delta: f64,
    /// Exact `Phi(after) - Phi(before)` from the trial re-solve;
    /// `None` when the trial solve failed.
    pub exact_phi_delta: Option<f64>,
    /// `|exact - predicted|` and its ratio to `|exact|`, when the trial
    /// solved.
    pub first_order_error: Option<f64>,
    pub first_order_error_rel: Option<f64>,
    pub accepted: bool,
    /// Why the step was kept or dropped, one short sentence.
    pub reason: String,
}

/// The outcome of one bounded planning search and its unapplied
/// proposal.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CapacityPlanOutcome {
    /// Summary of the baseline and the final accepted exact solve. The solve
    /// numbers refer to this planning operation's exact solve budget.
    pub baseline: CapacityPlanResultSummary,
    pub exact_verified_result: CapacityPlanResultSummary,
    /// `Phi` at the starting exact solve.
    pub baseline_phi: f64,
    /// `Phi` at the final accepted exact solve.
    pub final_phi: f64,
    /// The accumulated accepted rating changes: the unapplied proposal.
    pub proposal: Vec<RatingChange>,
    /// Total accepted movement `Σ |delta|`, MW, against the budget.
    pub spent_budget_mw: f64,
    /// Every trial in order, accepted and rejected.
    pub iterations: Vec<CapacityPlanIteration>,
    /// Exact OPF solves consumed, including the baseline.
    pub exact_solves: usize,
}

/// One exact result retained in the serializable planning record.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CapacityPlanResultSummary {
    pub phi: f64,
    pub declared_objective: f64,
    pub exact_solve: usize,
}

/// A completed planning operation with the final exact solve retained for
/// portable PowerIO solution emission. The solver workspace stays private.
pub struct CapacityPlanExecution {
    outcome: CapacityPlanOutcome,
    amended_instance: Arc<DcOpfInstance>,
    model: DcNetwork,
    solution: DcOpfSolution,
}

impl CapacityPlanExecution {
    pub fn outcome(&self) -> &CapacityPlanOutcome {
        &self.outcome
    }

    /// Consume the execution and emit the exact verified PowerIO solution
    /// without another OPF solve.
    pub fn into_solution(
        self,
        producer: impl Into<String>,
    ) -> Result<(CapacityPlanOutcome, powerio_prob::DcOpfSolution), String> {
        let solution = crate::emit::emit_dc_opf_solution(
            self.amended_instance,
            &self.model,
            &self.solution,
            producer,
        )?;
        Ok((self.outcome, solution))
    }
}

struct PlannedModel {
    outcome: CapacityPlanOutcome,
    model: DcNetwork,
    solution: DcOpfSolution,
}

/// A step must lower `Phi` by at least this much to be kept: below
/// it, an "improvement" is indistinguishable from interior point noise, and
/// accepting noise would spend budget on nothing.
const NORMALIZED_MIN_IMPROVEMENT: f64 = 1e-4;
/// Gradient entries below this magnitude read as flat: the dual signal of a
/// slack constraint is numerical residue, and a residue scaled step would
/// churn trials that cannot improve.
const NORMALIZED_GRADIENT_FLOOR: f64 = 1e-7;
/// Resolve one canonical PowerIO branch identity to its dense column.
fn branch_index(dc: &DcNetwork, identity: &str) -> Result<usize, String> {
    dc.branch_identities
        .iter()
        .position(|candidate| candidate == identity)
        .ok_or_else(|| format!("plan candidate branch {identity:?} is not in the model"))
}

/// Resolve one PowerIO bus ID to its dense index.
fn bus_index(dc: &DcNetwork, bus: BusId) -> Result<usize, String> {
    dc.bus_ids
        .iter()
        .position(|&candidate| candidate == bus.0)
        .ok_or_else(|| format!("objective bus {bus} is not in the model"))
}

/// `Phi` at a solution: the weighted marginal demand value over the resolved buses.
fn phi(dc: &DcNetwork, sol: &DcOpfSolution, weights: &[(usize, f64)]) -> f64 {
    let lmp = sol.nodal_marginal_values(dc.base_mva);
    weights.iter().map(|&(bus, w)| w * lmp[bus]).sum()
}

fn is_cancelled(cancel: Option<&Arc<AtomicBool>>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

/// Accepted capacity changes are positive by construction. Do not apply the
/// numerical comparison tolerance when counting or emitting them: doing so
/// would let a small accepted change alter the exact result while disappearing
/// from both the cardinality bound and the portable proposal.
fn has_capacity_increase(increase: f64) -> bool {
    increase > 0.0
}

/// Whether a remaining MW allowance can hold one complete increment. The
/// tolerance follows floating point scale, so it covers subtraction noise
/// without authorizing a small but real extra step past a bound.
fn remaining_allows_step(remaining: f64, increment: f64) -> bool {
    if increment <= remaining {
        return true;
    }
    let scale = remaining.abs().max(increment.abs());
    increment - remaining <= 64.0 * f64::EPSILON * scale
}

fn require_increment_multiple(name: &str, value: f64, increment: f64) -> Result<(), String> {
    let steps = value / increment;
    if (steps - steps.round()).abs() > 1e-9 * steps.abs().max(1.0) {
        return Err(format!("{name} must be a whole multiple of increment_mw"));
    }
    Ok(())
}

/// Run one bounded planning operation over a PowerIO instance. Every exact
/// solve is counted in `exact_solve_budget`, and the final accepted solve is
/// retained for emission.
pub fn plan_capacity(
    instance: Arc<DcOpfInstance>,
    spec: &CapacityPlanSpec,
) -> Result<CapacityPlanExecution, String> {
    plan_capacity_cancellable(instance, spec, None)
}

/// As [`plan_capacity`], with a flag checked before the baseline, between
/// sensitivity calculations, and by every exact OPF solve.
pub fn plan_capacity_cancellable(
    instance: Arc<DcOpfInstance>,
    spec: &CapacityPlanSpec,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<CapacityPlanExecution, String> {
    let model = DcNetwork::from_instance(&instance)?;
    let planned = plan_capacity_impl(&model, spec, cancel, None, |_| {})?;
    let mut amended_network = instance.network().clone();
    for change in &planned.outcome.proposal {
        let dense = branch_index(&planned.model, &change.branch)?;
        let source_row = planned.model.branch_source_rows[dense].ok_or_else(|| {
            format!(
                "plan candidate branch {:?} is a derived analysis row",
                change.branch
            )
        })?;
        amended_network.branches_mut()[source_row].rate_a =
            planned.model.fmax[dense] * planned.model.base_mva;
    }
    let amended_instance = Arc::new(
        Arc::unwrap_or_clone(instance)
            .with_network(amended_network)
            .map_err(|error| error.to_string())?,
    );
    Ok(CapacityPlanExecution {
        outcome: planned.outcome,
        amended_instance,
        model: planned.model,
        solution: planned.solution,
    })
}

#[cfg(test)]
fn plan_capacity_model(
    dc: &DcNetwork,
    spec: &CapacityPlanSpec,
) -> Result<CapacityPlanOutcome, String> {
    Ok(plan_capacity_impl(dc, spec, None, None, |_| {})?.outcome)
}

pub(crate) fn plan_capacity_from_exact(
    dc: &DcNetwork,
    solution: &DcOpfSolution,
    spec: &CapacityPlanSpec,
) -> Result<CapacityPlanOutcome, String> {
    Ok(plan_capacity_impl(dc, spec, None, Some(solution), |_| {})?.outcome)
}

fn plan_capacity_impl(
    dc: &DcNetwork,
    spec: &CapacityPlanSpec,
    cancel: Option<Arc<AtomicBool>>,
    initial_solution: Option<&DcOpfSolution>,
    exact_solve_completed: impl FnMut(usize),
) -> Result<PlannedModel, String> {
    plan_capacity_impl_with_solver(
        dc,
        spec,
        cancel,
        initial_solution,
        exact_solve_completed,
        dc_opf_cancellable,
    )
}

fn plan_capacity_impl_with_solver(
    dc: &DcNetwork,
    spec: &CapacityPlanSpec,
    cancel: Option<Arc<AtomicBool>>,
    initial_solution: Option<&DcOpfSolution>,
    mut exact_solve_completed: impl FnMut(usize),
    mut solve_exact: impl FnMut(&DcNetwork, Option<Arc<AtomicBool>>) -> Result<DcOpfSolution, String>,
) -> Result<PlannedModel, String> {
    if dc.allow_shed {
        return Err(
            "capacity planning requires an exact solve without undeclared load shedding".to_owned(),
        );
    }
    if dc.objective != powerio_matrix::PreparedObjective::NetworkGeneratorCost {
        return Err(
            "weighted_lmp requires the network_generator_cost PowerIO objective".to_owned(),
        );
    }
    let ImplicitObjective::WeightedLmp { weights } = &spec.objective;
    if weights.is_empty() {
        return Err("the objective names no buses".into());
    }
    if spec.candidates.is_empty() {
        return Err("the plan names no candidate branches".into());
    }
    // `<=` alone would wave NaN through; a NaN step or budget must reject.
    if !spec.increment_mw.is_finite() || spec.increment_mw <= 0.0 {
        return Err("increment_mw must be positive".into());
    }
    if !spec.budget_mw.is_finite() || spec.budget_mw <= 0.0 {
        return Err("budget_mw must be positive".into());
    }
    if !spec.max_increase_per_branch_mw.is_finite() || spec.max_increase_per_branch_mw <= 0.0 {
        return Err("max_increase_per_branch_mw must be positive".into());
    }
    if spec.exact_solve_budget == 0 {
        return Err("exact_solve_budget must include at least the baseline solve".into());
    }
    require_increment_multiple("budget_mw", spec.budget_mw, spec.increment_mw)?;
    require_increment_multiple(
        "max_increase_per_branch_mw",
        spec.max_increase_per_branch_mw,
        spec.increment_mw,
    )?;

    let bus_weights: Vec<(usize, f64)> = weights
        .iter()
        .map(|w| {
            if !w.weight.is_finite() {
                return Err(format!("objective weight for bus {} is not finite", w.bus));
            }
            bus_index(dc, w.bus).map(|i| (i, w.weight))
        })
        .collect::<Result<_, _>>()?;
    {
        let mut seen = std::collections::BTreeSet::new();
        for (&(dense_bus, _), term) in bus_weights.iter().zip(weights) {
            if !seen.insert(dense_bus) {
                return Err(format!(
                    "objective bus {} appears twice; combine its weights into one term",
                    term.bus
                ));
            }
        }
    }
    // Positive scaling of every weight changes Phi's units, not the planning
    // problem. Normalize only the numerical decision thresholds so w and k*w
    // accept the same trials; reported objective values retain their declared
    // scale.
    let objective_scale = bus_weights
        .iter()
        .map(|&(_, weight)| weight.abs())
        .fold(0.0_f64, f64::max);
    let objective_scale = if objective_scale > 0.0 {
        objective_scale
    } else {
        1.0
    };
    if spec.max_changed_lines == 0 {
        return Err("max_changed_lines must be at least one".to_owned());
    }
    let candidates: Vec<usize> = spec
        .candidates
        .iter()
        .map(|identity| branch_index(dc, identity))
        .collect::<Result<_, _>>()?;
    {
        let mut seen = std::collections::BTreeSet::new();
        for (&col, key) in candidates.iter().zip(&spec.candidates) {
            if !seen.insert(col) {
                return Err(format!("candidate branch {key} appears twice"));
            }
        }
    }
    for (&dense, identity) in candidates.iter().zip(&spec.candidates) {
        if dc.branch_source_rows[dense].is_none() {
            return Err(format!(
                "candidate branch {identity:?} is a derived analysis row"
            ));
        }
    }
    if spec.max_changed_lines > candidates.len() {
        return Err(format!(
            "max_changed_lines {} exceeds the {} candidate branches",
            spec.max_changed_lines,
            candidates.len()
        ));
    }

    if is_cancelled(cancel.as_ref()) {
        return Err("capacity planning cancelled".into());
    }
    let mut current = dc.clone();
    let mut sol = if let Some(solution) = initial_solution {
        solution.clone()
    } else {
        solve_exact(&current, cancel.clone())?
    };
    let mut exact_solves = 1usize;
    let mut current_exact_solve = exact_solves;
    exact_solve_completed(exact_solves);
    let baseline_phi = phi(&current, &sol, &bus_weights);
    let baseline_declared_objective = sol.objective;

    let mut cumulative = vec![0.0f64; candidates.len()];
    let mut budget_left = spec.budget_mw;
    let mut iterations: Vec<CapacityPlanIteration> = Vec::new();
    let mut phi_current = baseline_phi;

    'search: while exact_solves < spec.exact_solve_budget {
        if is_cancelled(cancel.as_ref()) {
            return Err("capacity planning cancelled".into());
        }
        if !remaining_allows_step(budget_left, spec.increment_mw) {
            break;
        }
        // The implicit gradient at the current exact solution, scaled to
        // outer objective units per MW.
        let kkt = DcKkt::new(&current, &sol);
        let scale = kkt.unit_scale(Operand::Price(Power::Active), Parameter::LineLimit);
        let per_unit = weighted_sensitivity(
            &kkt,
            Operand::Price(Power::Active),
            &bus_weights,
            Parameter::LineLimit,
            Some(&candidates),
        )
        .map_err(|e| e.to_string())?;
        let gradient: Vec<f64> = per_unit.iter().map(|g| g * scale).collect();
        let reported: Vec<GradientEntry> = spec
            .candidates
            .iter()
            .zip(&gradient)
            .map(|(key, &value)| GradientEntry {
                branch: key.clone(),
                value,
            })
            .collect();

        // Movable candidates: descent direction with room inside the bounds.
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by(|&a, &b| {
            gradient[a]
                .partial_cmp(&gradient[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let line_limit = spec.max_changed_lines;
        let changed_lines = cumulative
            .iter()
            .filter(|&&increase| has_capacity_increase(increase))
            .count();
        let mut attempted = false;
        let mut accepted = false;
        for &candidate_slot in &order {
            if exact_solves == spec.exact_solve_budget {
                break;
            }
            let g = gradient[candidate_slot];
            if g / objective_scale >= -NORMALIZED_GRADIENT_FLOOR {
                continue;
            }
            let already_changed = has_capacity_increase(cumulative[candidate_slot]);
            if !already_changed && changed_lines >= line_limit {
                continue;
            }
            let room = spec.max_increase_per_branch_mw - cumulative[candidate_slot];
            if !remaining_allows_step(room, spec.increment_mw) {
                continue;
            }
            let increase = spec.increment_mw;
            let predicted = g * increase;
            let trial_changes = vec![RatingChange {
                branch: spec.candidates[candidate_slot].clone(),
                delta_mw: increase,
            }];
            let mut trial = current.clone();
            let dense_branch = candidates[candidate_slot];
            trial.fmax[dense_branch] += increase / trial.base_mva;
            attempted = true;
            exact_solves += 1;
            let outcome = solve_exact(&trial, cancel.clone());
            exact_solve_completed(exact_solves);

            match outcome {
                Ok(trial_sol) => {
                    let phi_new = phi(&trial, &trial_sol, &bus_weights);
                    let exact = phi_new - phi_current;
                    let error = (exact - predicted).abs();
                    let rel = if exact.abs() > f64::EPSILON {
                        Some(error / exact.abs())
                    } else {
                        None
                    };
                    let accepted_trial = exact / objective_scale < -NORMALIZED_MIN_IMPROVEMENT;
                    iterations.push(CapacityPlanIteration {
                        gradient: reported.clone(),
                        delta_mw: trial_changes,
                        predicted_phi_delta: predicted,
                        exact_phi_delta: Some(exact),
                        first_order_error: Some(error),
                        first_order_error_rel: rel,
                        accepted: accepted_trial,
                        reason: if accepted_trial {
                            "exact re-solve improves the objective".into()
                        } else {
                            "exact re-solve does not improve the objective".into()
                        },
                    });
                    if accepted_trial {
                        cumulative[candidate_slot] += increase;
                        budget_left -= increase;
                        current = trial;
                        sol = trial_sol;
                        phi_current = phi_new;
                        current_exact_solve = exact_solves;
                        accepted = true;
                        break;
                    }
                }
                Err(err) => {
                    if is_cancelled(cancel.as_ref()) {
                        return Err("capacity planning cancelled".into());
                    }
                    iterations.push(CapacityPlanIteration {
                        gradient: reported.clone(),
                        delta_mw: trial_changes,
                        predicted_phi_delta: predicted,
                        exact_phi_delta: None,
                        first_order_error: None,
                        first_order_error_rel: None,
                        accepted: false,
                        reason: format!("trial solve failed: {err}"),
                    });
                }
            }
        }
        if accepted {
            continue;
        }
        if !attempted {
            iterations.push(CapacityPlanIteration {
                gradient: reported,
                delta_mw: Vec::new(),
                predicted_phi_delta: 0.0,
                exact_phi_delta: None,
                first_order_error: None,
                first_order_error_rel: None,
                accepted: false,
                reason: "no candidate capacity increase is available within the gradient, budget, per branch, and global line bounds".into(),
            });
        }
        break 'search;
    }

    let proposal: Vec<RatingChange> = spec
        .candidates
        .iter()
        .zip(&cumulative)
        .filter(|(_, &d)| has_capacity_increase(d))
        .map(|(key, &d)| RatingChange {
            branch: key.clone(),
            delta_mw: d,
        })
        .collect();

    let final_declared_objective = sol.objective;
    Ok(PlannedModel {
        outcome: CapacityPlanOutcome {
            baseline: CapacityPlanResultSummary {
                phi: baseline_phi,
                declared_objective: baseline_declared_objective,
                exact_solve: 1,
            },
            exact_verified_result: CapacityPlanResultSummary {
                phi: phi_current,
                declared_objective: final_declared_objective,
                exact_solve: current_exact_solve,
            },
            baseline_phi,
            final_phi: phi_current,
            proposal,
            spent_budget_mw: cumulative.iter().map(|d| d.abs()).sum(),
            iterations,
            exact_solves,
        },
        model: current,
        solution: sol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case3;
    use crate::sens::Mode;

    /// case3 with line 1-2 squeezed until it binds, so the LMPs split and a
    /// rating change has a nonzero dual footprint.
    fn congested_case3() -> DcNetwork {
        let mut dc = parse_case3();
        // The unconstrained solve routes ~0.398 pu over this line; capping
        // at 0.36 binds it and splits the nodal values (load bus ~17.9 vs
        // ~11.5 uncongested) while the problem remains feasible without load
        // shedding.
        dc.fmax[0] = 0.36;
        dc
    }

    fn plan_spec() -> CapacityPlanSpec {
        CapacityPlanSpec {
            objective: ImplicitObjective::WeightedLmp {
                weights: vec![BusWeight {
                    bus: BusId(2),
                    weight: 1.0,
                }],
            },
            candidates: ["branches:0", "branches:1", "branches:2"]
                .map(str::to_owned)
                .to_vec(),
            max_increase_per_branch_mw: 15.0,
            budget_mw: 20.0,
            increment_mw: 5.0,
            max_changed_lines: 1,
            exact_solve_budget: 8,
        }
    }

    #[test]
    fn weighted_lmp_gradient_matches_finite_differences() {
        let dc = congested_case3();
        let sol = dc_opf_cancellable(&dc, None).expect("base solve");
        let weights: Vec<(usize, f64)> = vec![(0, 0.25), (1, 1.0), (2, 0.5)];

        let kkt = DcKkt::new(&dc, &sol);
        let scale = kkt.unit_scale(Operand::Price(Power::Active), Parameter::LineLimit);
        let cols: Vec<usize> = (0..dc.m).collect();
        let grad: Vec<f64> = weighted_sensitivity(
            &kkt,
            Operand::Price(Power::Active),
            &weights,
            Parameter::LineLimit,
            Some(&cols),
        )
        .expect("adjoint gradient")
        .iter()
        .map(|g| g * scale)
        .collect();

        // The same gradient row by row through the dense driver: the one
        // functional adjoint must agree with the full matrix contraction.
        let dense = crate::sens::sensitivity(
            &kkt,
            Operand::Price(Power::Active),
            Parameter::LineLimit,
            Some(&cols),
            Mode::Forward,
        )
        .expect("dense sensitivity");
        for (e, &g) in grad.iter().enumerate() {
            let contracted: f64 = weights
                .iter()
                .map(|&(bus, w)| w * dense.values[bus][e] * scale)
                .sum();
            assert!(
                (g - contracted).abs() < 1e-9,
                "column {e}: adjoint {g} vs contracted {contracted}"
            );
        }

        // Central finite differences on the exact solve, in MW.
        let h_mw = 0.5;
        for (e, &g) in grad.iter().enumerate() {
            let mut up = dc.clone();
            up.fmax[e] += h_mw / up.base_mva;
            let mut down = dc.clone();
            down.fmax[e] -= h_mw / down.base_mva;
            let phi_up = phi(&up, &dc_opf_cancellable(&up, None).expect("up"), &weights);
            let phi_down = phi(
                &down,
                &dc_opf_cancellable(&down, None).expect("down"),
                &weights,
            );
            let fd = (phi_up - phi_down) / (2.0 * h_mw);
            // An active set change inside the stencil voids the comparison;
            // the congested line's dual is strictly interior here, so the
            // stencil stays on one face for every column.
            assert!(
                (g - fd).abs() < 1e-3 * (1.0 + fd.abs()),
                "branch {e}: adjoint {g} vs finite difference {fd}"
            );
        }
    }

    #[test]
    fn planning_relieves_congestion_within_bounds_and_budget() {
        let dc = congested_case3();
        let spec = plan_spec();
        let outcome = plan_capacity_model(&dc, &spec).expect("plan");

        assert!(
            outcome.final_phi < outcome.baseline_phi,
            "phi must improve: {} -> {}",
            outcome.baseline_phi,
            outcome.final_phi
        );
        assert!(outcome.spent_budget_mw <= spec.budget_mw + 1e-9);
        assert!(!outcome.proposal.is_empty());
        assert!(outcome.proposal.len() <= spec.max_changed_lines);
        for change in &outcome.proposal {
            assert!(change.delta_mw >= spec.increment_mw - 1e-9);
            assert!(change.delta_mw <= spec.max_increase_per_branch_mw + 1e-9);
            let steps = change.delta_mw / spec.increment_mw;
            assert!((steps - steps.round()).abs() < 1e-9);
        }
        assert!(outcome.exact_solves <= spec.exact_solve_budget);
        assert_eq!(outcome.baseline.exact_solve, 1);
        assert!(outcome.exact_verified_result.exact_solve <= outcome.exact_solves);
        assert_eq!(outcome.exact_verified_result.phi, outcome.final_phi);
        // Every accepted iteration must record the exact outcome and its
        // first order error.
        for it in outcome.iterations.iter().filter(|it| it.accepted) {
            assert!(it.exact_phi_delta.unwrap() < 0.0);
            assert!(it.first_order_error.is_some());
        }
        // The model handed in is untouched.
        assert!((dc.fmax[0] - 0.36).abs() < 1e-12);
    }

    #[test]
    fn positive_weight_scaling_does_not_change_the_proposal() {
        let dc = congested_case3();
        let base_spec = plan_spec();
        let base = plan_capacity_model(&dc, &base_spec).expect("base plan");
        let base_proposal: Vec<_> = base
            .proposal
            .iter()
            .map(|change| (&change.branch, change.delta_mw))
            .collect();
        let base_acceptance: Vec<_> = base.iterations.iter().map(|it| it.accepted).collect();

        for factor in [1e-6, 1e6] {
            let mut scaled_spec = base_spec.clone();
            let ImplicitObjective::WeightedLmp { weights } = &mut scaled_spec.objective;
            for term in weights {
                term.weight *= factor;
            }
            let scaled = plan_capacity_model(&dc, &scaled_spec).expect("scaled plan");
            let scaled_proposal: Vec<_> = scaled
                .proposal
                .iter()
                .map(|change| (&change.branch, change.delta_mw))
                .collect();
            let scaled_acceptance: Vec<_> =
                scaled.iterations.iter().map(|it| it.accepted).collect();
            assert_eq!(scaled_proposal, base_proposal, "weight scale {factor}");
            assert_eq!(scaled_acceptance, base_acceptance, "weight scale {factor}");
            assert_eq!(
                scaled.exact_solves, base.exact_solves,
                "weight scale {factor}"
            );
        }
    }

    #[test]
    fn execution_emits_the_exact_solution_on_the_amended_instance() {
        let mut network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        network.branches_mut()[0].rate_a = 36.0;
        let instance = Arc::new(DcOpfInstance::from_network(network).expect("instance"));
        let mut spec = plan_spec();
        spec.candidates = vec!["branches:0".into()];
        spec.max_changed_lines = 1;
        let execution = plan_capacity(instance, &spec).expect("plan");
        let change = execution
            .outcome()
            .proposal
            .first()
            .expect("accepted change");
        assert_eq!(change.branch, "branches:0");
        let expected_rating = 36.0 + change.delta_mw;

        let (outcome, solution) = execution.into_solution("tellegen test").expect("emit");
        assert_eq!(
            solution.objective(),
            outcome.exact_verified_result.declared_objective
        );
        assert_eq!(
            solution.instance().network().branches()[0].rate_a,
            expected_rating
        );
        assert_eq!(
            outcome.exact_verified_result.exact_solve,
            outcome.exact_solves
        );
    }

    #[test]
    fn sparse_candidates_keep_their_powerio_identities() {
        let dc = congested_case3();
        let mut dense_spec = plan_spec();
        dense_spec.max_changed_lines = dense_spec.candidates.len();
        dense_spec.exact_solve_budget = 2;
        let dense = plan_capacity_model(&dc, &dense_spec).expect("dense plan");
        let dense_gradient = &dense.iterations[0].gradient;

        let mut spec = plan_spec();
        spec.candidates = vec!["branches:2".into(), "branches:0".into()];
        spec.max_changed_lines = 2;
        spec.exact_solve_budget = 2;
        let outcome = plan_capacity_model(&dc, &spec).expect("plan");
        let gradient = &outcome.iterations[0].gradient;
        assert_eq!(gradient[0].branch, "branches:2");
        assert_eq!(gradient[1].branch, "branches:0");
        assert!((gradient[0].value - dense_gradient[2].value).abs() < 1e-12);
        assert!((gradient[1].value - dense_gradient[0].value).abs() < 1e-12);
        assert!(outcome
            .proposal
            .iter()
            .all(|change| spec.candidates.contains(&change.branch)));
    }

    #[test]
    fn planning_rejects_an_unknown_candidate_and_an_empty_objective() {
        let dc = congested_case3();
        let bad = CapacityPlanSpec {
            objective: ImplicitObjective::WeightedLmp {
                weights: vec![BusWeight {
                    bus: BusId(2),
                    weight: 1.0,
                }],
            },
            candidates: vec!["branches:99".into()],
            max_increase_per_branch_mw: 10.0,
            budget_mw: 10.0,
            increment_mw: 5.0,
            max_changed_lines: 1,
            exact_solve_budget: 2,
        };
        let err = plan_capacity_model(&dc, &bad).unwrap_err();
        assert!(err.contains("not in the model"), "got: {err}");

        let empty = CapacityPlanSpec {
            objective: ImplicitObjective::WeightedLmp {
                weights: Vec::new(),
            },
            candidates: vec!["branches:0".into()],
            max_increase_per_branch_mw: 10.0,
            budget_mw: 10.0,
            increment_mw: 5.0,
            max_changed_lines: 1,
            exact_solve_budget: 2,
        };
        let err = plan_capacity_model(&dc, &empty).unwrap_err();
        assert!(err.contains("names no buses"), "got: {err}");
    }

    #[test]
    fn planning_rejects_duplicate_candidates_and_objective_buses() {
        let dc = congested_case3();
        let mut spec = plan_spec();
        spec.candidates = vec!["branches:0".into(), "branches:0".into()];
        let err = plan_capacity_model(&dc, &spec).unwrap_err();
        assert!(err.contains("appears twice"), "got: {err}");

        let mut spec = plan_spec();
        spec.objective = ImplicitObjective::WeightedLmp {
            weights: vec![
                BusWeight {
                    bus: BusId(2),
                    weight: 1.0,
                },
                BusWeight {
                    bus: BusId(2),
                    weight: -0.5,
                },
            ],
        };
        let err = plan_capacity_model(&dc, &spec).unwrap_err();
        assert!(err.contains("combine its weights"), "got: {err}");
    }

    #[test]
    fn an_uncongestible_direction_stops_without_spending_budget() {
        // Every line generously rated: no dual, zero gradient, no movement.
        let dc = parse_case3();
        let mut spec = plan_spec();
        spec.max_changed_lines = spec.candidates.len();
        spec.exact_solve_budget = 4;
        let outcome = plan_capacity_model(&dc, &spec).expect("plan");
        assert!(outcome.proposal.is_empty());
        assert!(outcome.spent_budget_mw.abs() < 1e-9);
        assert!((outcome.final_phi - outcome.baseline_phi).abs() < 1e-9);
    }

    #[test]
    fn exact_solve_budget_counts_the_baseline_and_every_trial() {
        let dc = congested_case3();
        let mut spec = plan_spec();
        spec.exact_solve_budget = 2;
        let outcome = plan_capacity_model(&dc, &spec).expect("plan");
        assert_eq!(outcome.exact_solves, 2);
        assert_eq!(outcome.iterations.len(), 1);
    }

    #[test]
    fn sub_tolerance_accepted_increases_still_count_as_changes() {
        assert!(has_capacity_increase(1e-10));
        assert!(!has_capacity_increase(0.0));
    }

    #[test]
    fn bound_tolerance_never_authorizes_a_complete_extra_small_step() {
        assert!(remaining_allows_step(0.1 - f64::EPSILON, 0.1));
        assert!(!remaining_allows_step(0.0, 1e-10));
        assert!(!remaining_allows_step(-f64::EPSILON, 1e-10));
    }

    #[test]
    fn an_exact_trial_without_material_improvement_is_rejected() {
        let dc = congested_case3();
        let original_limits = dc.fmax.clone();
        let mut spec = plan_spec();
        spec.increment_mw = 1e-6;
        spec.budget_mw = 1e-6;
        spec.max_increase_per_branch_mw = 1e-6;
        spec.exact_solve_budget = 2;
        let outcome = plan_capacity_model(&dc, &spec).expect("plan");
        assert_eq!(outcome.exact_solves, 2);
        assert_eq!(outcome.iterations.len(), 1);
        assert!(outcome.iterations[0].exact_phi_delta.is_some());
        assert!(!outcome.iterations[0].accepted);
        assert!(outcome.proposal.is_empty());
        assert_eq!(dc.fmax, original_limits);
    }

    #[test]
    fn an_infeasible_exact_trial_is_recorded_and_does_not_mutate_input() {
        let dc = congested_case3();
        let original_limits = dc.fmax.clone();
        let mut calls = 0usize;
        let planned = plan_capacity_impl_with_solver(
            &dc,
            &plan_spec(),
            None,
            None,
            |_| {},
            |model, cancel| {
                calls += 1;
                if calls == 1 {
                    dc_opf_cancellable(model, cancel)
                } else {
                    Err("infeasible trial fixture".to_owned())
                }
            },
        )
        .expect("a failed trial is an audited planning outcome");
        assert_eq!(planned.outcome.exact_solves, 2);
        assert_eq!(calls, 2);
        assert_eq!(planned.outcome.iterations.len(), 1);
        assert!(planned.outcome.iterations[0]
            .reason
            .contains("infeasible trial fixture"));
        assert!(planned.outcome.iterations[0].exact_phi_delta.is_none());
        assert!(!planned.outcome.iterations[0].accepted);
        assert!(planned.outcome.proposal.is_empty());
        assert_eq!(dc.fmax, original_limits);
    }

    #[test]
    fn an_unservable_instance_returns_no_plan_or_solution() {
        let mut network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        for generator in network.generators_mut() {
            generator.pmax = 40.0;
        }
        let original = network.to_json().expect("network JSON");
        let instance = Arc::new(DcOpfInstance::from_network(network).expect("instance"));
        let error = plan_capacity(instance.clone(), &plan_spec())
            .err()
            .expect("an unservable baseline must not produce a proposal");
        assert!(error.to_ascii_lowercase().contains("infeasible"), "{error}");
        assert_eq!(
            instance.network().to_json().expect("network JSON"),
            original
        );
    }

    #[test]
    fn cancellation_is_checked_before_the_baseline_solve() {
        let dc = congested_case3();
        let cancel = Arc::new(AtomicBool::new(true));
        let err = plan_capacity_impl(&dc, &plan_spec(), Some(cancel), None, |_| {})
            .err()
            .expect("cancelled before baseline");
        assert!(err.contains("cancelled"), "got: {err}");
    }

    #[test]
    fn cancellation_between_exact_solves_leaves_the_input_unchanged() {
        let dc = congested_case3();
        let original_limits = dc.fmax.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let observed = cancel.clone();
        let err = plan_capacity_impl(&dc, &plan_spec(), Some(cancel), None, move |solves| {
            if solves == 1 {
                observed.store(true, Ordering::Relaxed);
            }
        })
        .err()
        .expect("cancelled after baseline");
        assert!(err.contains("cancelled"), "got: {err}");
        assert_eq!(dc.fmax, original_limits);
    }

    #[test]
    fn non_integral_capacity_bounds_are_rejected() {
        let dc = congested_case3();
        let mut spec = plan_spec();
        spec.max_increase_per_branch_mw = 12.0;
        let err = plan_capacity_model(&dc, &spec).unwrap_err();
        assert!(err.contains("whole multiple"), "got: {err}");
    }

    #[test]
    fn zero_cardinality_and_noneconomic_objectives_are_rejected() {
        let dc = congested_case3();
        let mut spec = plan_spec();
        spec.max_changed_lines = 0;
        let error = plan_capacity_model(&dc, &spec).unwrap_err();
        assert!(error.contains("at least one"), "got: {error}");

        let mut feasibility = dc;
        feasibility.objective = powerio_matrix::PreparedObjective::Feasibility;
        let error = plan_capacity_model(&feasibility, &plan_spec()).unwrap_err();
        assert!(error.contains("network_generator_cost"), "got: {error}");
    }
}
