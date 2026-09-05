//! Deterministic gradient-guided search with exact, unapplied candidate states.

use crate::objective::{
    DecisionSpace, DemandConstraint, Intervention, ObjectiveResult, StudyObjective,
};
use crate::{NetworkEdit, Study};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchOptions {
    #[cfg_attr(feature = "schema", schemars(range(min = 0, max = 256)))]
    pub max_solves: usize,
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = 32)))]
    pub beam_width: usize,
    #[cfg_attr(feature = "schema", schemars(range(min = 0, max = 256)))]
    pub max_iterations: usize,
    #[cfg_attr(feature = "schema", schemars(range(min = 0)))]
    pub min_improvement: f64,
}

impl SearchOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_solves > 256
            || self.beam_width == 0
            || self.beam_width > 32
            || self.max_iterations > 256
            || !self.min_improvement.is_finite()
            || self.min_improvement < 0.0
        {
            return Err("search requires at most 256 solves/iterations, beam 1..32 and finite nonnegative improvement".into());
        }
        Ok(())
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_solves: 24,
            beam_width: 4,
            max_iterations: 8,
            min_improvement: 1e-7,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExactCandidate {
    pub input: String,
    pub solution: String,
    pub view: String,
    pub changes: Vec<f64>,
    pub value: f64,
}

impl ExactCandidate {
    pub fn capture(study: &Study, changes: Vec<f64>, value: f64) -> Result<Self, String> {
        Ok(Self {
            input: study.save_instance_module()?,
            solution: study.save_exact_module()?,
            view: serde_json::to_string(study.solution()).map_err(|e| e.to_string())?,
            changes,
            value,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchTrial {
    pub iteration: usize,
    pub parent_trial: Option<usize>,
    pub changes: Vec<f64>,
    pub predicted_value: f64,
    pub exact: Option<ExactCandidate>,
    pub accepted: bool,
    pub failure: Option<String>,
    pub active_constraints_added: Vec<String>,
    pub active_constraints_removed: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    pub baseline_value: f64,
    pub best: Option<ExactCandidate>,
    pub trials: Vec<SearchTrial>,
    pub directions: Vec<ObjectiveResult>,
    pub solve_count: usize,
    pub termination: String,
}

/// Search from an already solved goal anchor. The caller accounts for its creation
/// separately; every candidate solve here consumes one unit of `max_solves`.
pub fn explore(
    study: &Study,
    objective: &StudyObjective,
    space: &DecisionSpace,
    start_changes: &[f64],
    options: &SearchOptions,
    mut cancelled: impl FnMut() -> bool,
) -> Result<SearchResult, String> {
    finish_synchronous(explore_async(
        study,
        objective,
        space,
        start_changes,
        options,
        || std::future::ready(cancelled()),
    ))
}

/// Yield between exact trials so hosts can process cancellation without losing
/// the completed evidence. Each solve remains an indivisible numerical trial.
pub async fn explore_async<F: std::future::Future<Output = bool>>(
    study: &Study,
    objective: &StudyObjective,
    space: &DecisionSpace,
    start_changes: &[f64],
    options: &SearchOptions,
    mut checkpoint: impl FnMut() -> F,
) -> Result<SearchResult, String> {
    space.validate(study.formulation())?;
    objective.validate(space)?;
    options.validate()?;
    if start_changes.len() != space.variables.len() || start_changes.iter().any(|x| !x.is_finite())
    {
        return Err("search start requires one finite change per decision".into());
    }
    study.check_decision_state(space, start_changes)?;
    let baseline = study.objective_value(objective, space, start_changes)?;
    let mut current = study.fork();
    let mut changes = start_changes.to_vec();
    let mut value = baseline;
    let mut result = SearchResult {
        baseline_value: baseline,
        best: None,
        trials: Vec::new(),
        directions: Vec::new(),
        solve_count: 0,
        termination: "iteration_limit".into(),
    };
    if space.feasible(&changes, 1e-7) {
        result.best = Some(ExactCandidate::capture(&current, changes.clone(), value)?);
    }
    let mut visited = BTreeSet::new();
    visited.insert(change_key(&changes));
    let mut parent_trial = None;
    'iterations: for iteration in 0..options.max_iterations {
        if checkpoint().await {
            result.termination = "cancelled".into();
            break;
        }
        if result.solve_count >= options.max_solves {
            result.termination = "solve_budget".into();
            break;
        }
        let direction = match current.objective_gradient(objective, space, &changes) {
            Ok(direction) => direction,
            Err(error) => {
                result.termination = format!("derivative_unavailable: {error}");
                break;
            }
        };
        let gradient = direction
            .gradient
            .iter()
            .map(|g| g.value)
            .collect::<Vec<_>>();
        result.directions.push(direction);
        let mut candidates = feasible_moves(space, &changes, &gradient, result.best.is_none());
        candidates.retain(|x| !visited.contains(&change_key(x)));
        candidates.sort_by(|a, b| {
            prediction(value, &gradient, &changes, a)
                .total_cmp(&prediction(value, &gradient, &changes, b))
                .then_with(|| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        });
        if candidates.is_empty() {
            result.termination = "no_feasible_moves".into();
            break;
        }
        let active_before = current.active_constraints();
        let mut winner: Option<(Study, Vec<f64>, f64, usize)> = None;
        for candidate in candidates.into_iter().take(options.beam_width) {
            if checkpoint().await {
                result.termination = "cancelled".into();
                break;
            }
            if result.solve_count >= options.max_solves {
                result.termination = "solve_budget".into();
                break;
            }
            visited.insert(change_key(&candidate));
            let predicted = prediction(value, &gradient, &changes, &candidate);
            let mut trial = SearchTrial {
                iteration,
                parent_trial,
                changes: candidate.clone(),
                predicted_value: predicted,
                exact: None,
                accepted: false,
                failure: None,
                active_constraints_added: Vec::new(),
                active_constraints_removed: Vec::new(),
            };
            let mut tested = current.fork();
            let edits = decision_edits(space, &candidate);
            // A failed exact solve consumes the same budget as a successful one.
            result.solve_count += 1;
            let evaluated = tested.replace_edits(&edits).and_then(|_| {
                let objective_value = tested.objective_value(objective, space, &candidate)?;
                let exact = ExactCandidate::capture(&tested, candidate.clone(), objective_value)?;
                Ok((objective_value, exact))
            });
            match evaluated {
                Ok((exact_value, exact)) => {
                    let after = tested.active_constraints();
                    trial.active_constraints_added =
                        after.difference(&active_before).cloned().collect();
                    trial.active_constraints_removed =
                        active_before.difference(&after).cloned().collect();
                    trial.exact = Some(exact);
                    let improves =
                        result.best.is_none() || exact_value < value - options.min_improvement;
                    if improves
                        && winner.as_ref().is_none_or(|(_, _, best, _)| {
                            exact_value < *best - options.min_improvement
                        })
                    {
                        winner = Some((tested, candidate, exact_value, result.trials.len()));
                    }
                }
                Err(error) => trial.failure = Some(error),
            }
            result.trials.push(trial);
        }
        if let Some((next, next_changes, next_value, index)) = winner {
            result.trials[index].accepted = true;
            result.best = result.trials[index].exact.clone();
            current = next;
            changes = next_changes;
            value = next_value;
            parent_trial = Some(index);
        } else if result.termination == "iteration_limit" {
            result.termination = "no_verified_improvement".into();
            break;
        }
        if matches!(result.termination.as_str(), "cancelled" | "solve_budget") {
            break 'iterations;
        }
    }
    Ok(result)
}

/// Synchronous callers supply ready checkpoint futures, so one poll completes.
pub(crate) fn finish_synchronous<T>(
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => Err("a synchronous Study checkpoint cannot suspend".into()),
    }
}

pub fn decision_edits(space: &DecisionSpace, changes: &[f64]) -> Vec<NetworkEdit> {
    space
        .variables
        .iter()
        .zip(changes)
        .filter(|(_, x)| x.abs() > 1e-12)
        .map(|(v, &x)| match v.intervention {
            Intervention::BranchRating => NetworkEdit::AdjustBranchRating {
                branch: v.element.clone(),
                delta_mw: x,
            },
            Intervention::ActiveDemand => NetworkEdit::AddLoad {
                bus: v.element.clone(),
                p_mw: x,
            },
        })
        .collect()
}

fn prediction(value: f64, gradient: &[f64], old: &[f64], new: &[f64]) -> f64 {
    value
        + gradient
            .iter()
            .zip(old)
            .zip(new)
            .map(|((&g, &a), &b)| g * (b - a))
            .sum::<f64>()
}
fn change_key(changes: &[f64]) -> Vec<u64> {
    changes
        .iter()
        .map(|x| if *x == 0.0 { 0 } else { x.to_bits() })
        .collect()
}

fn feasible_moves(
    space: &DecisionSpace,
    current: &[f64],
    gradient: &[f64],
    needs_placement: bool,
) -> Vec<Vec<f64>> {
    let mut moves = Vec::new();
    let demand = space
        .variables
        .iter()
        .enumerate()
        .filter(|(_, v)| v.intervention == Intervention::ActiveDemand)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if needs_placement {
        if let Some(DemandConstraint::Placement { increase_mw }) = space.demand {
            let mut order = demand.clone();
            order.sort_by(|&a, &b| gradient[a].total_cmp(&gradient[b]).then(a.cmp(&b)));
            // A different first allocation tests distinct corners of the bounded simplex.
            for &first in order.iter().take(32) {
                let mut point = current.to_vec();
                for &index in &demand {
                    point[index] = 0.0;
                }
                let mut remaining = increase_mw;
                for index in
                    std::iter::once(first).chain(order.iter().copied().filter(|&i| i != first))
                {
                    let v = &space.variables[index];
                    let amount =
                        (remaining.min(v.upper) / v.increment + 1e-10).floor() * v.increment;
                    point[index] = amount;
                    remaining -= amount;
                }
                if space.feasible(&point, 1e-7) {
                    moves.push(point);
                }
            }
        }
        return moves;
    }
    let mut ratings = space
        .variables
        .iter()
        .enumerate()
        .filter(|(_, v)| v.intervention == Intervention::BranchRating)
        .flat_map(|(index, v)| {
            [-1.0, 1.0]
                .into_iter()
                .map(move |sign| (index, sign, gradient[index] * sign * v.increment))
        })
        .collect::<Vec<_>>();
    ratings.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
    for (index, sign, _) in ratings.into_iter().take(256) {
        let mut point = current.to_vec();
        point[index] += sign * space.variables[index].increment;
        if space.feasible(&point, 1e-7) {
            moves.push(point);
        }
    }
    // Bound the pair pool independently of network size before allocating moves.
    let mut receivers = demand.clone();
    receivers.sort_by(|&a, &b| gradient[a].total_cmp(&gradient[b]).then(a.cmp(&b)));
    let mut donors = demand;
    donors.sort_by(|&a, &b| gradient[b].total_cmp(&gradient[a]).then(a.cmp(&b)));
    for &from in donors.iter().take(16) {
        for &to in receivers.iter().take(16) {
            if from == to {
                continue;
            }
            let a = space.variables[from].increment;
            let b = space.variables[to].increment;
            let step = (1..=64)
                .map(|n| a * f64::from(n))
                .find(|x| (x / b - (x / b).round()).abs() < 1e-9);
            if let Some(step) = step {
                let mut point = current.to_vec();
                point[from] -= step;
                point[to] += step;
                if space.feasible(&point, 1e-7) {
                    moves.push(point);
                }
            }
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objective::{DecisionVariable, ObservableWeight};
    use crate::{ElementKey, Operand, Power, Problem};

    fn fixture() -> (Study, StudyObjective, DecisionSpace) {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        net.branches_mut()[0].rate_a = 35.0;
        let input = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(net),
        ))
        .unwrap();
        let study = Study::new(&input, Problem::DcOpf).unwrap();
        let objective = StudyObjective::WeightedObservable {
            operand: Operand::Price(Power::Active),
            weights: vec![ObservableWeight {
                element: ElementKey::Id(2),
                weight: 1.0,
            }],
        };
        let space = DecisionSpace {
            variables: vec![DecisionVariable {
                id: "line".into(),
                element: 1.into(),
                intervention: Intervention::BranchRating,
                lower: 0.0,
                upper: 10.0,
                increment: 1.0,
                budget_weight: 1.0,
            }],
            total_budget: 10.0,
            max_changed_elements: 1,
            demand: None,
        };
        (study, objective, space)
    }

    #[test]
    fn search_improves_exact_goal_and_leaves_applied_input_unchanged() {
        let (study, objective, space) = fixture();
        let before = study.save_instance_module().unwrap();
        let result = explore(
            &study,
            &objective,
            &space,
            &[0.0],
            &SearchOptions::default(),
            || false,
        )
        .unwrap();
        assert!(result.best.as_ref().unwrap().value < result.baseline_value - 0.01);
        assert_eq!(study.save_instance_module().unwrap(), before);
        assert_eq!(result.solve_count, result.trials.len());
        assert!(result.solve_count <= 24);
        assert!(result
            .trials
            .iter()
            .all(|t| space.feasible(&t.changes, 1e-7)));
        for trial in result.trials.iter().filter(|t| t.accepted) {
            let exact = trial.exact.as_ref().unwrap();
            let restored = Study::new(&exact.input, Problem::DcOpf).unwrap();
            assert!(
                (restored
                    .objective_value(&objective, &space, &exact.changes)
                    .unwrap()
                    - exact.value)
                    .abs()
                    < 1e-5
            );
        }
    }

    #[test]
    fn budgets_cancellation_and_mismatched_start_are_explicit() {
        let (study, objective, space) = fixture();
        let one = SearchOptions {
            max_solves: 1,
            ..Default::default()
        };
        let result = explore(&study, &objective, &space, &[0.0], &one, || false).unwrap();
        assert_eq!(result.solve_count, 1);
        let cancelled = explore(&study, &objective, &space, &[0.0], &one, || true).unwrap();
        assert_eq!(cancelled.solve_count, 0);
        assert_eq!(cancelled.termination, "cancelled");
        assert!(explore(&study, &objective, &space, &[1.0], &one, || false)
            .unwrap_err()
            .contains("starting electrical state"));
    }

    #[test]
    fn demand_moves_respect_placement_and_redistribution_totals() {
        let (_, _, mut space) = fixture();
        space.variables = [2, 3]
            .into_iter()
            .map(|id| DecisionVariable {
                id: format!("demand{id}"),
                element: id.into(),
                intervention: Intervention::ActiveDemand,
                lower: 0.0,
                upper: 10.0,
                increment: 2.0,
                budget_weight: 1.0,
            })
            .collect();
        space.max_changed_elements = 2;
        space.demand = Some(DemandConstraint::Placement { increase_mw: 6.0 });
        let moves = feasible_moves(&space, &[0.0, 0.0], &[1.0, 2.0], true);
        assert_eq!(moves, vec![vec![6.0, 0.0], vec![0.0, 6.0]]);
        assert!(moves.iter().all(|x| space.feasible(x, 1e-7)));
        for v in &mut space.variables {
            v.lower = -10.0;
        }
        space.demand = Some(DemandConstraint::Redistribution);
        let moves = feasible_moves(&space, &[0.0, 0.0], &[1.0, 2.0], false);
        assert_eq!(moves, vec![vec![2.0, -2.0], vec![-2.0, 2.0]]);
    }
    #[test]
    fn exact_demand_search_preserves_totals_and_records_infeasible_trials() {
        let (study, objective, mut space) = fixture();
        space.variables = [2, 3]
            .into_iter()
            .map(|id| DecisionVariable {
                id: format!("demand{id}"),
                element: id.into(),
                intervention: Intervention::ActiveDemand,
                lower: -10.0,
                upper: 10.0,
                increment: 2.0,
                budget_weight: 1.0,
            })
            .collect();
        space.total_budget = 20.0;
        space.max_changed_elements = 2;
        space.demand = Some(DemandConstraint::Redistribution);
        let before = study.save_instance_module().unwrap();
        let shifted = explore(
            &study,
            &objective,
            &space,
            &[0.0, 0.0],
            &SearchOptions::default(),
            || false,
        )
        .unwrap();
        let best = shifted.best.unwrap();
        assert!(best.value < shifted.baseline_value - 0.01);
        assert!(best.changes.iter().sum::<f64>().abs() < 1e-9);
        assert_eq!(study.save_instance_module().unwrap(), before);
        for variable in &mut space.variables {
            variable.lower = 0.0;
        }
        space.demand = Some(DemandConstraint::Placement { increase_mw: 6.0 });
        let placed = explore(
            &study,
            &objective,
            &space,
            &[0.0, 0.0],
            &SearchOptions::default(),
            || false,
        )
        .unwrap();
        assert!((placed.best.unwrap().changes.iter().sum::<f64>() - 6.0).abs() < 1e-9);
        for variable in &mut space.variables {
            variable.upper = 10_000.0;
        }
        space.total_budget = 10_000.0;
        space.demand = Some(DemandConstraint::Placement {
            increase_mw: 10_000.0,
        });
        let failed = explore(
            &study,
            &objective,
            &space,
            &[0.0, 0.0],
            &SearchOptions {
                max_solves: 2,
                ..Default::default()
            },
            || false,
        )
        .unwrap();
        assert!(failed.best.is_none());
        assert_eq!(failed.solve_count, 2);
        assert_eq!(failed.trials.len(), 2);
        assert!(failed
            .trials
            .iter()
            .all(|t| t.failure.is_some() && t.exact.is_none()));
        assert_eq!(study.save_instance_module().unwrap(), before);
    }

    #[test]
    fn crossing_a_congestion_boundary_records_the_exact_constraint_change() {
        let (study, objective, mut space) = fixture();
        space.variables[0].upper = 100.0;
        space.variables[0].increment = 100.0;
        space.total_budget = 100.0;
        let result = explore(
            &study,
            &objective,
            &space,
            &[0.0],
            &SearchOptions {
                max_solves: 1,
                ..Default::default()
            },
            || false,
        )
        .unwrap();
        assert_eq!(result.solve_count, 1);
        let trial = &result.trials[0];
        assert!(trial.accepted);
        assert!(!trial.active_constraints_removed.is_empty());
        let exact = trial.exact.as_ref().unwrap().value;
        assert!((exact - trial.predicted_value).abs() > 0.01);
    }
}
