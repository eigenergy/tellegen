//! Shared application operations for browser, CLI, and agent adapters.

use crate::document::*;
use crate::exploration::{decision_edits, ExactCandidate, SearchOptions};
use crate::objective::{DecisionSpace, Intervention, StudyObjective};
use crate::{Problem, Study};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateStudy {
    pub id: String,
    pub title: String,
    pub input: String,
    #[cfg_attr(feature = "schema", schemars(schema_with = "study_formulation_schema"))]
    pub formulation: Problem,
    pub request: String,
    pub interpretation: String,
    pub objective: StudyObjective,
    pub decisions: DecisionSpace,
    pub success_value: Option<f64>,
}

#[cfg(feature = "schema")]
fn study_formulation_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let formulations = if cfg!(feature = "conic") {
        vec!["dcopf", "acpf", "socwr"]
    } else {
        vec!["dcopf", "acpf"]
    };
    schemars::json_schema!({ "type": "string", "enum": formulations })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyRequest {
    pub expected_revision: u64,
    pub operation: StudyOperation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StudyOperation {
    Inspect {
        state: String,
    },
    Branch {
        state: String,
        rationale: String,
    },
    ReviseGoal {
        goal: GoalRevision,
    },
    Compare {
        left: String,
        right: String,
        goal: String,
    },
    Propose {
        state: String,
        goal: String,
        options: SearchOptions,
        rationale: String,
    },
    RecordEvidence {
        state: String,
        goal: String,
        sensitivity: bool,
        assessed_recommendation: Option<String>,
        rationale: String,
        evidence: serde_json::Value,
    },
    /// The calling UI or native user action must authorize this exact binding.
    Apply {
        proposal: String,
        state: String,
        goal: String,
        base_state: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub goal: String,
    pub left: String,
    pub right: String,
    pub left_value: f64,
    pub right_value: f64,
    pub improvement: f64,
    pub left_view: crate::SolveResponse,
    pub right_view: crate::SolveResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyOperationResult {
    pub summary: StudySummary,
    pub experiment: Option<String>,
    pub inspected_view: Option<crate::SolveResponse>,
    pub comparison: Option<Comparison>,
}

pub fn create_study(request: CreateStudy) -> Result<StudyBundle, String> {
    request.decisions.validate(request.formulation)?;
    request.objective.validate(&request.decisions)?;
    let study = Study::new(&request.input, request.formulation)?;
    let changes = vec![0.0; request.decisions.variables.len()];
    study.check_decision_state(&request.decisions, &changes)?;
    let value = study.objective_value(&request.objective, &request.decisions, &changes)?;
    let exact = ExactCandidate::capture(&study, changes, value)?;
    let mut bundle = StudyBundle::empty(request.id, request.title)?;
    let root = bundle.capture_state(&exact, None, "Starting point".into())?;
    bundle.document.applied_state = Some(root.clone());
    bundle.document.inspected_state = Some(root.clone());
    let goal = bundle.revise_goal(GoalRevision {
        parent: None,
        anchor_state: root.clone(),
        request: request.request,
        interpretation: request.interpretation,
        objective: request.objective,
        decisions: request.decisions,
        success_value: request.success_value,
    })?;
    let evidence = bundle.add_artifact(
        ArtifactKind::Evidence,
        serde_json::json!({ "operation": "create_study", "exact_value": value, "solve_count": 1 })
            .to_string(),
    )?;
    bundle.add_experiment(ExperimentRecord {
        start_state: Some(root),
        goal: Some(goal),
        kind: ExperimentKind::Counterfactual,
        rationale: "Establish the exact starting operating point".into(),
        evidence: vec![evidence],
        trials: vec![],
        result_states: vec![],
        assessed_recommendation: None,
        solve_count: 1,
        termination: "completed".into(),
    })?;
    bundle.validate()?;
    Ok(bundle)
}

/// Apply one operation to a validated copy. Persistence adapters store the returned
/// document before publishing it as the current application state.
pub fn execute_study(
    bundle: &mut StudyBundle,
    request: StudyRequest,
    mut cancelled: impl FnMut() -> bool,
) -> Result<StudyOperationResult, String> {
    bundle.check_revision(request.expected_revision)?;
    bundle.validate()?;
    let (experiment, comparison) =
        bundle.transaction(request.expected_revision, |next| match request.operation {
            StudyOperation::Inspect { state } => {
                next.inspect(&state)?;
                Ok((None, None))
            }
            StudyOperation::Branch { state, rationale } => {
                next.inspect(&state)?;
                let goal = next
                    .document
                    .active_goal
                    .clone()
                    .ok_or("branch requires an active goal")?;
                let experiment = next.add_experiment(ExperimentRecord {
                    start_state: Some(state),
                    goal: Some(goal),
                    kind: ExperimentKind::Inspection,
                    rationale,
                    evidence: vec![],
                    trials: vec![],
                    result_states: vec![],
                    assessed_recommendation: None,
                    solve_count: 0,
                    termination: "branch_selected".into(),
                })?;
                Ok((Some(experiment), None))
            }
            StudyOperation::ReviseGoal { goal } => {
                next.revise_goal(goal)?;
                Ok((None, None))
            }
            StudyOperation::Compare { left, right, goal } => {
                let comparison = compare_states(next, &left, &right, &goal)?;
                let evidence = next.add_artifact(
                    ArtifactKind::Evidence,
                    serde_json::to_string(&comparison).map_err(|e| e.to_string())?,
                )?;
                let experiment = next.add_experiment(ExperimentRecord {
                    start_state: Some(left),
                    goal: Some(goal),
                    kind: ExperimentKind::Inspection,
                    rationale: "Compare exact saved candidates under the selected goal revision"
                        .into(),
                    evidence: vec![evidence],
                    trials: vec![],
                    result_states: vec![],
                    assessed_recommendation: None,
                    solve_count: 0,
                    termination: "completed".into(),
                })?;
                Ok((Some(experiment), Some(comparison)))
            }
            StudyOperation::RecordEvidence {
                state,
                goal,
                sensitivity,
                assessed_recommendation,
                rationale,
                evidence,
            } => {
                let artifact = next.add_artifact(ArtifactKind::Evidence, evidence.to_string())?;
                let experiment = next.add_experiment(ExperimentRecord {
                    start_state: Some(state),
                    goal: Some(goal),
                    kind: if assessed_recommendation.is_some() {
                        ExperimentKind::Challenge
                    } else if sensitivity {
                        ExperimentKind::Sensitivity
                    } else {
                        ExperimentKind::Inspection
                    },
                    rationale,
                    evidence: vec![artifact],
                    trials: vec![],
                    result_states: vec![],
                    assessed_recommendation,
                    solve_count: 0,
                    termination: "completed".into(),
                })?;
                Ok((Some(experiment), None))
            }
            StudyOperation::Propose {
                state,
                goal,
                options,
                rationale,
            } => {
                options.validate()?;
                let experiment = propose(next, &state, &goal, &options, rationale, &mut cancelled)?;
                Ok((Some(experiment), None))
            }
            StudyOperation::Apply {
                proposal,
                state,
                goal,
                base_state,
            } => {
                let record = next
                    .document
                    .experiments
                    .get(&proposal)
                    .ok_or("proposal is unavailable")?;
                if !matches!(record.kind, ExperimentKind::Planning)
                    || record.goal.as_ref() != Some(&goal)
                    || record.start_state.as_ref() != Some(&base_state)
                    || next.document.active_goal.as_ref() != Some(&goal)
                    || next.document.recommended_state.as_ref() != Some(&state)
                    || !record.result_states.contains(&state)
                {
                    return Err(
                        "proposal approval no longer matches the goal, base and recommended state"
                            .into(),
                    );
                }
                next.add_decision(DecisionRecord {
                    experiment: proposal.clone(),
                    state: Some(state.clone()),
                    choice: DecisionKind::Apply,
                    rationale: "Explicitly applied the proposal bound to this goal and base state"
                        .into(),
                    evidence: record.evidence.clone(),
                })?;
                next.document.applied_state = Some(state.clone());
                next.inspect(&state)?;
                Ok((Some(proposal), None))
            }
        })?;
    operation_result(bundle, experiment, comparison)
}

fn operation_result(
    bundle: &StudyBundle,
    experiment: Option<String>,
    comparison: Option<Comparison>,
) -> Result<StudyOperationResult, String> {
    let inspected_view = bundle
        .document
        .inspected_state
        .as_ref()
        .map(|state| bundle.state_view(state))
        .transpose()?;
    Ok(StudyOperationResult {
        summary: bundle.summary(8),
        experiment,
        inspected_view,
        comparison,
    })
}

/// Execute a proposal with host checkpoints between exact solves. Cancellation
/// completes the planning record with every finished trial and its best candidate.
pub async fn execute_study_async<F: std::future::Future<Output = bool>>(
    bundle: &mut StudyBundle,
    request: StudyRequest,
    checkpoint: impl FnMut() -> F,
) -> Result<StudyOperationResult, String> {
    let StudyOperation::Propose {
        state,
        goal,
        options,
        rationale,
    } = request.operation
    else {
        return execute_study(bundle, request, || false);
    };
    bundle.check_revision(request.expected_revision)?;
    bundle.validate()?;
    options.validate()?;
    let mut candidate = bundle.clone();
    let experiment = propose_async(
        &mut candidate,
        &state,
        &goal,
        &options,
        rationale,
        checkpoint,
    )
    .await?;
    bundle.transaction(request.expected_revision, |next| {
        *next = candidate;
        Ok(())
    })?;
    operation_result(bundle, Some(experiment), None)
}

pub fn compare_states(
    bundle: &StudyBundle,
    left: &str,
    right: &str,
    goal_id: &str,
) -> Result<Comparison, String> {
    let goal = bundle
        .document
        .goals
        .get(goal_id)
        .ok_or("goal revision is unavailable")?;
    let evaluate = |state: &str| -> Result<(f64, crate::SolveResponse), String> {
        let view = bundle.state_view(state)?;
        let network = state_network(bundle, state)?;
        let changes = state_changes(bundle, &goal.anchor_state, state, &goal.decisions)?;
        let values = goal
            .decisions
            .variables
            .iter()
            .zip(changes)
            .map(|(v, x)| (v.id.clone(), x))
            .collect();
        let value = goal.objective.evaluate(&view, &network, &values)?.value;
        Ok((value, view))
    };
    let (left_value, left_view) = evaluate(left)?;
    let (right_value, right_view) = evaluate(right)?;
    Ok(Comparison {
        goal: goal_id.into(),
        left: left.into(),
        right: right.into(),
        left_value,
        right_value,
        improvement: left_value - right_value,
        left_view,
        right_view,
    })
}

pub(crate) fn state_network(
    bundle: &StudyBundle,
    id: &str,
) -> Result<powerio::BalancedNetwork, String> {
    let state = bundle
        .document
        .states
        .get(id)
        .ok_or("state is unavailable")?;
    let input = crate::ir::deserialize_module(
        &bundle
            .artifacts
            .get(&state.input)
            .ok_or("state input is unavailable")?
            .text,
    )?;
    match input.into_value() {
        powerio::PioValue::DcOpfInstance(i) => Ok(i.network().clone()),
        powerio::PioValue::AcPfInstance(i) => Ok(i.network().clone()),
        powerio::PioValue::AcOpfInstance(i) => Ok(i.network().clone()),
        _ => Err("state does not contain a supported problem instance".into()),
    }
}

pub fn state_changes(
    bundle: &StudyBundle,
    anchor: &str,
    state: &str,
    space: &DecisionSpace,
) -> Result<Vec<f64>, String> {
    let base = state_network(bundle, anchor)?;
    let current = state_network(bundle, state)?;
    let value = |network: &powerio::BalancedNetwork,
                 v: &crate::objective::DecisionVariable|
     -> Result<f64, String> {
        let id = crate::objective::source_element_id(
            network,
            v.intervention.parameter().axis(),
            &v.element,
        )?;
        match v.intervention {
            Intervention::BranchRating => network
                .branches()
                .get(id.checked_sub(1).ok_or("branch ids start at 1")?)
                .map(|b| b.rate_a)
                .ok_or("branch is unavailable".into()),
            Intervention::ActiveDemand => {
                if !network.buses().iter().any(|b| b.id.0 == id) {
                    return Err("bus is unavailable".into());
                }
                Ok(network
                    .loads()
                    .iter()
                    .filter(|load| load.in_service && load.bus.0 == id)
                    .map(|load| load.p)
                    .sum())
            }
        }
    };
    space
        .variables
        .iter()
        .map(|v| Ok(value(&current, v)? - value(&base, v)?))
        .collect()
}

fn propose(
    bundle: &mut StudyBundle,
    state_id: &str,
    goal_id: &str,
    options: &SearchOptions,
    rationale: String,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<String, String> {
    crate::exploration::finish_synchronous(propose_async(
        bundle,
        state_id,
        goal_id,
        options,
        rationale,
        || std::future::ready(cancelled()),
    ))
}

async fn propose_async<F: std::future::Future<Output = bool>>(
    bundle: &mut StudyBundle,
    state_id: &str,
    goal_id: &str,
    options: &SearchOptions,
    rationale: String,
    mut checkpoint: impl FnMut() -> F,
) -> Result<String, String> {
    if bundle.document.active_goal.as_deref() != Some(goal_id) {
        return Err("proposal requires the active goal revision".into());
    }
    let goal = bundle
        .document
        .goals
        .get(goal_id)
        .ok_or("goal is unavailable")?
        .clone();
    let state = bundle
        .document
        .states
        .get(state_id)
        .ok_or("starting state is unavailable")?
        .clone();
    let anchor = bundle
        .document
        .states
        .get(&goal.anchor_state)
        .ok_or("goal anchor is unavailable")?;
    if state.formulation != anchor.formulation {
        return Err("goal and starting state use different formulations".into());
    }
    let changes = state_changes(bundle, &goal.anchor_state, state_id, &goal.decisions)?;
    let input = bundle
        .artifacts
        .get(&anchor.input)
        .ok_or("goal input is unavailable")?
        .text
        .clone();
    let start_input = bundle
        .artifacts
        .get(&state.input)
        .ok_or("starting input is unavailable")?
        .text
        .clone();
    let mut solve_count = 0;
    let run = async {
        if checkpoint().await {
            return Err("cancelled".into());
        }
        if options.max_solves == 0 {
            return Err("solve_budget".into());
        }
        solve_count += 1;
        let mut study = Study::new(&input, state.formulation)?;
        if changes.iter().any(|x| x.abs() > 1e-10) {
            if checkpoint().await {
                return Err("cancelled".into());
            }
            if solve_count >= options.max_solves {
                return Err("solve_budget".into());
            }
            solve_count += 1;
            study.replace_edits(&decision_edits(&goal.decisions, &changes))?;
        }
        let canonical = |text: &str| -> Result<String, String> {
            let module = crate::ir::deserialize_module(text)?;
            crate::ir::serialize_module(&powerio::PioModule::new(module.into_value()))
        };
        if canonical(&study.save_instance_module()?)? != canonical(&start_input)? {
            return Err(
                "starting state contains changes outside the selected goal's decisions".into(),
            );
        }
        let remaining = SearchOptions {
            max_solves: options.max_solves - solve_count,
            ..options.clone()
        };
        crate::exploration::explore_async(
            &study,
            &goal.objective,
            &goal.decisions,
            &changes,
            &remaining,
            checkpoint,
        )
        .await
    }
    .await;
    let mut record = ExperimentRecord {
        start_state: Some(state_id.into()),
        goal: Some(goal_id.into()),
        kind: ExperimentKind::Planning,
        rationale,
        evidence: vec![],
        trials: vec![],
        result_states: vec![],
        assessed_recommendation: None,
        solve_count,
        termination: "completed".into(),
    };
    let mut recommendation = None;
    match run {
        Err(error) => {
            record.termination = error;
        }
        Ok(search) => {
            record.solve_count += search.solve_count;
            record.termination = search.termination;
            let evidence = bundle.add_artifact(ArtifactKind::Evidence, serde_json::json!({ "directions": search.directions,
                "baseline_value": search.baseline_value, "solve_count": record.solve_count, "termination": record.termination }).to_string())?;
            record.evidence.push(evidence);
            let mut trial_states: Vec<Option<String>> = Vec::new();
            for (index, trial) in search.trials.into_iter().enumerate() {
                let parent = trial
                    .parent_trial
                    .and_then(|i| trial_states.get(i).cloned().flatten())
                    .unwrap_or_else(|| state_id.into());
                let exact_value = trial.exact.as_ref().map(|e| e.value);
                let saved = trial
                    .exact
                    .as_ref()
                    .map(|exact| {
                        bundle.capture_state(exact, Some(parent), format!("Trial {}", index + 1))
                    })
                    .transpose()?;
                if let Some(saved) = &saved {
                    record.result_states.push(saved.clone());
                }
                if trial.accepted {
                    recommendation = saved.clone();
                }
                trial_states.push(saved.clone());
                let evidence = bundle.add_artifact(ArtifactKind::Evidence, serde_json::json!({
                    "iteration": trial.iteration, "active_constraints_added": trial.active_constraints_added,
                    "active_constraints_removed": trial.active_constraints_removed,
                    "prediction_error": exact_value.map(|v| v - trial.predicted_value), "failure": trial.failure }).to_string())?;
                record.trials.push(TrialRecord {
                    changes: trial.changes,
                    predicted_value: Some(trial.predicted_value),
                    exact_value,
                    state: saved,
                    accepted: trial.accepted,
                    failure: trial.failure,
                    evidence: vec![evidence],
                });
            }
            if recommendation.is_none() && search.best.is_some() {
                recommendation = Some(state_id.into());
                record.result_states.push(state_id.into());
            }
        }
    }
    let evidence = record.evidence.clone();
    let id = bundle.add_experiment(record)?;
    if let Some(state) = recommendation {
        bundle.add_decision(DecisionRecord {
            experiment: id.clone(),
            state: Some(state.clone()),
            choice: DecisionKind::Recommend,
            rationale:
                "Best feasible candidate verified by exact solves within the declared search budget"
                    .into(),
            evidence,
        })?;
        bundle.document.recommended_state = Some(state);
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objective::{DecisionVariable, ObservableWeight};
    use crate::{Operand, Power};

    fn fixture() -> StudyBundle {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        net.branches_mut()[0].rate_a = 35.0;
        let input = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(net),
        ))
        .unwrap();
        create_study(CreateStudy {
            id: "study".into(),
            title: "Price study".into(),
            input,
            formulation: Problem::DcOpf,
            request: "Lower bus 2 price".into(),
            interpretation: "Minimize bus 2 LMP with a 10 MW line upgrade budget".into(),
            objective: StudyObjective::WeightedObservable {
                operand: Operand::Price(Power::Active),
                weights: vec![ObservableWeight {
                    element: 2.into(),
                    weight: 1.0,
                }],
            },
            decisions: DecisionSpace {
                variables: vec![DecisionVariable {
                    id: "line1".into(),
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
            },
            success_value: None,
        })
        .unwrap()
    }

    #[test]
    fn cancellation_preserves_completed_trials_and_best_candidate() {
        let mut bundle = fixture();
        let base = bundle.document.applied_state.clone().unwrap();
        let goal = bundle.document.active_goal.clone().unwrap();
        let mut checkpoints = 0;
        let result = execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 0,
                operation: StudyOperation::Propose {
                    state: base.clone(),
                    goal,
                    options: SearchOptions::default(),
                    rationale: "Explore upgrades".into(),
                },
            },
            || {
                checkpoints += 1;
                checkpoints >= 4
            },
        )
        .unwrap();
        let record = &bundle.document.experiments[&result.experiment.unwrap()];
        assert_eq!(record.termination, "cancelled");
        assert_eq!(record.solve_count, 2);
        assert_eq!(record.trials.len(), 1);
        assert!(record.trials[0].exact_value.is_some());
        assert!(record.trials[0].accepted);
        assert_ne!(bundle.document.recommended_state.as_ref(), Some(&base));
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&base));
        let restored = StudyBundle::import(&bundle.export().unwrap()).unwrap();
        assert_eq!(restored.document.revision, 1);
        assert_eq!(
            restored.document.recommended_state,
            bundle.document.recommended_state
        );
    }

    #[test]
    fn asynchronous_checkpoints_preserve_synchronous_search_results() {
        struct YieldOnce(bool);
        impl std::future::Future for YieldOnce {
            type Output = bool;
            fn poll(
                mut self: std::pin::Pin<&mut Self>,
                context: &mut std::task::Context<'_>,
            ) -> std::task::Poll<bool> {
                if self.0 {
                    std::task::Poll::Ready(false)
                } else {
                    self.0 = true;
                    context.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            }
        }
        let mut asynchronous = fixture();
        let mut synchronous = asynchronous.clone();
        let request = StudyRequest {
            expected_revision: 0,
            operation: StudyOperation::Propose {
                state: asynchronous.document.applied_state.clone().unwrap(),
                goal: asynchronous.document.active_goal.clone().unwrap(),
                options: SearchOptions {
                    max_solves: 5,
                    ..Default::default()
                },
                rationale: "Explore upgrades".into(),
            },
        };
        execute_study(&mut synchronous, request.clone(), || false).unwrap();
        {
            let future = execute_study_async(&mut asynchronous, request, || YieldOnce(false));
            let mut future = std::pin::pin!(future);
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            loop {
                if let std::task::Poll::Ready(result) =
                    std::future::Future::poll(future.as_mut(), &mut context)
                {
                    result.unwrap();
                    break;
                }
            }
        }
        assert_eq!(
            asynchronous.export().unwrap(),
            synchronous.export().unwrap()
        );
    }

    #[test]
    fn proposal_compare_branch_and_explicit_apply_share_one_document() {
        let mut bundle = fixture();
        let base = bundle.document.applied_state.clone().unwrap();
        let goal = bundle.document.active_goal.clone().unwrap();
        let result = execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 0,
                operation: StudyOperation::Propose {
                    state: base.clone(),
                    goal: goal.clone(),
                    options: SearchOptions {
                        max_solves: 5,
                        ..Default::default()
                    },
                    rationale: "Test gradient-ranked upgrades".into(),
                },
            },
            || false,
        )
        .unwrap();
        let proposal = result.experiment.unwrap();
        let recommended = bundle.document.recommended_state.clone().unwrap();
        assert_ne!(recommended, base);
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&base));
        assert!(bundle.document.experiments[&proposal].solve_count <= 5);
        let comparison = compare_states(&bundle, &base, &recommended, &goal).unwrap();
        assert!(comparison.improvement > 0.0);
        let count = bundle.document.states.len();
        execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 1,
                operation: StudyOperation::Branch {
                    state: recommended.clone(),
                    rationale: "Continue this candidate".into(),
                },
            },
            || false,
        )
        .unwrap();
        assert_eq!(bundle.document.states.len(), count);
        let serialized = bundle.export().unwrap();
        bundle = StudyBundle::import(&serialized).unwrap();
        assert!(execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 1,
                operation: StudyOperation::Apply {
                    proposal: proposal.clone(),
                    state: recommended.clone(),
                    goal: goal.clone(),
                    base_state: base.clone()
                }
            },
            || false
        )
        .is_err());
        execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 2,
                operation: StudyOperation::Apply {
                    proposal,
                    state: recommended.clone(),
                    goal,
                    base_state: base,
                },
            },
            || false,
        )
        .unwrap();
        assert_eq!(bundle.document.applied_state, Some(recommended));
    }

    #[test]
    fn goal_revision_invalidates_proposal_and_failed_search_is_recorded() {
        let mut bundle = fixture();
        let base = bundle.document.applied_state.clone().unwrap();
        let goal = bundle.document.active_goal.clone().unwrap();
        let result = execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 0,
                operation: StudyOperation::Propose {
                    state: base.clone(),
                    goal: goal.clone(),
                    options: SearchOptions {
                        max_solves: 0,
                        ..Default::default()
                    },
                    rationale: "Zero budget".into(),
                },
            },
            || false,
        )
        .unwrap();
        let experiment = &bundle.document.experiments[&result.experiment.unwrap()];
        assert_eq!(experiment.solve_count, 0);
        assert_eq!(experiment.termination, "solve_budget");
        let result = execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 1,
                operation: StudyOperation::Propose {
                    state: base.clone(),
                    goal: goal.clone(),
                    options: SearchOptions {
                        max_solves: 4,
                        ..Default::default()
                    },
                    rationale: "Upgrade".into(),
                },
            },
            || false,
        )
        .unwrap();
        let state = bundle.document.recommended_state.clone().unwrap();
        let proposal = result.experiment.unwrap();
        let mut revised = bundle.document.goals[&goal].clone();
        revised.parent = Some(goal.clone());
        revised.interpretation = "Revised target".into();
        execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 2,
                operation: StudyOperation::ReviseGoal { goal: revised },
            },
            || false,
        )
        .unwrap();
        let before = bundle.export().unwrap();
        assert!(execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: 3,
                operation: StudyOperation::Apply {
                    proposal,
                    state,
                    goal,
                    base_state: base
                }
            },
            || false
        )
        .is_err());
        assert_eq!(bundle.export().unwrap(), before);
    }
}
