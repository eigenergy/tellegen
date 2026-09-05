//! Shared application operations for browser, CLI, and agent adapters.

use crate::document::*;
use crate::exploration::{decision_edits, ExactCandidate, SearchOptions};
use crate::objective::{DecisionSpace, Intervention, StudyObjective};
use crate::{ElementKey, Problem, Study};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateStudy {
    pub id: String,
    pub title: String,
    pub input: String,
    /// Original network data before interactive edits, when available.
    #[serde(default)]
    pub base_input: Option<String>,
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
    EditDemand {
        state: String,
        goal: String,
        #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 4096)))]
        changes: Vec<DemandAdjustment>,
        rationale: String,
    },
    RestoreBase {
        state: String,
        goal: String,
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
pub struct DemandAdjustment {
    pub bus: ElementKey,
    /// Signed increment to the selected state's demand, in MW.
    pub delta_mw: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DemandChange {
    pub bus: ElementKey,
    pub base_mw: f64,
    pub demand_mw: f64,
    pub delta_mw: f64,
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
    #[serde(default)]
    pub demand_changes: Option<Vec<DemandChange>>,
}

pub fn create_study(request: CreateStudy) -> Result<StudyBundle, String> {
    request.decisions.validate(request.formulation)?;
    request.objective.validate(&request.decisions)?;
    let base_input = request.base_input.as_deref().unwrap_or(&request.input);
    same_bus_identities(&input_network(base_input)?, &input_network(&request.input)?)?;
    let study = Study::new(&request.input, request.formulation)?;
    let changes = vec![0.0; request.decisions.variables.len()];
    study.check_decision_state(&request.decisions, &changes)?;
    let value = study.objective_value(&request.objective, &request.decisions, &changes)?;
    let exact = ExactCandidate::capture(&study, changes, value)?;
    let mut bundle = StudyBundle::empty(request.id, request.title)?;
    bundle.document.base_input =
        Some(bundle.add_artifact(ArtifactKind::PowerioIr, base_input.into())?);
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
            StudyOperation::EditDemand {
                state,
                goal,
                changes,
                rationale,
            } => {
                let id = counterfactual(
                    next,
                    &state,
                    &goal,
                    Some(changes),
                    rationale,
                    &mut cancelled,
                )?;
                Ok((Some(id), None))
            }
            StudyOperation::RestoreBase {
                state,
                goal,
                rationale,
            } => {
                let id = counterfactual(next, &state, &goal, None, rationale, &mut cancelled)?;
                Ok((Some(id), None))
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
                if !matches!(
                    record.kind,
                    ExperimentKind::Planning | ExperimentKind::Counterfactual
                ) || record.goal.as_ref() != Some(&goal)
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
        demand_changes: bundle
            .document
            .inspected_state
            .as_deref()
            .map(|state| demand_changes(bundle, state))
            .transpose()?
            .flatten(),
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
    input_network(
        &bundle
            .artifacts
            .get(&state.input)
            .ok_or("state input is unavailable")?
            .text,
    )
}

pub(crate) fn input_network(text: &str) -> Result<powerio::BalancedNetwork, String> {
    match crate::ir::deserialize_module(text)?.into_value() {
        powerio::PioValue::BalancedNetwork(n) => Ok(n),
        powerio::PioValue::DcOpfInstance(i) => Ok(i.network().clone()),
        powerio::PioValue::AcPfInstance(i) => Ok(i.network().clone()),
        powerio::PioValue::AcOpfInstance(i) => Ok(i.network().clone()),
        _ => Err("input must contain a balanced network or supported problem instance".into()),
    }
}

fn same_bus_identities(
    base: &powerio::BalancedNetwork,
    current: &powerio::BalancedNetwork,
) -> Result<(), String> {
    let ids = |net: &powerio::BalancedNetwork| {
        net.buses()
            .iter()
            .map(|b| (b.id.0, b.uid.clone()))
            .collect::<std::collections::BTreeSet<_>>()
    };
    if ids(base) != ids(current) {
        return Err("base case and selected state have different bus identities".into());
    }
    Ok(())
}

/// Sparse cumulative MW changes relative to the retained network data.
pub fn demand_changes(
    bundle: &StudyBundle,
    state: &str,
) -> Result<Option<Vec<DemandChange>>, String> {
    let Some(base) = &bundle.document.base_input else {
        return Ok(None);
    };
    let base = input_network(
        &bundle
            .artifacts
            .get(base)
            .ok_or("base case input is unavailable")?
            .text,
    )?;
    let current = state_network(bundle, state)?;
    same_bus_identities(&base, &current)?;
    let demand = |net: &powerio::BalancedNetwork| {
        let mut values = std::collections::BTreeMap::<usize, f64>::new();
        for load in net.loads().iter().filter(|l| l.in_service) {
            *values.entry(load.bus.0).or_default() += load.p;
        }
        values
    };
    let original = demand(&base);
    let selected = demand(&current);
    Ok(Some(
        current
            .buses()
            .iter()
            .filter_map(|b| {
                let base_mw = original.get(&b.id.0).copied().unwrap_or_default();
                let demand_mw = selected.get(&b.id.0).copied().unwrap_or_default();
                let delta_mw = demand_mw - base_mw;
                (delta_mw.abs() > 1e-9).then(|| DemandChange {
                    bus: b
                        .uid
                        .clone()
                        .map(ElementKey::Uid)
                        .unwrap_or(ElementKey::Id(b.id.0 as i64)),
                    base_mw,
                    demand_mw,
                    delta_mw,
                })
            })
            .collect(),
    ))
}

pub fn state_changes(
    bundle: &StudyBundle,
    anchor: &str,
    state: &str,
    space: &DecisionSpace,
) -> Result<Vec<f64>, String> {
    let base = state_network(bundle, anchor)?;
    let current = state_network(bundle, state)?;
    network_changes(&base, &current, space)
}

fn network_changes(
    base: &powerio::BalancedNetwork,
    current: &powerio::BalancedNetwork,
    space: &DecisionSpace,
) -> Result<Vec<f64>, String> {
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
        .map(|v| Ok(value(current, v)? - value(base, v)?))
        .collect()
}

fn counterfactual(
    bundle: &mut StudyBundle,
    state_id: &str,
    goal_id: &str,
    adjustments: Option<Vec<DemandAdjustment>>,
    rationale: String,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<String, String> {
    if bundle.document.active_goal.as_deref() != Some(goal_id) {
        return Err("operation requires the active goal revision".into());
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
        .ok_or("state is unavailable")?
        .clone();
    if bundle.document.states[&goal.anchor_state].formulation != state.formulation {
        return Err("goal and selected state use different formulations".into());
    }
    let anchor = state_network(bundle, &goal.anchor_state)?;
    let current = state_network(bundle, state_id)?;
    let mut changes = network_changes(&anchor, &current, &goal.decisions)?;
    let restore = adjustments.is_none();
    let mut edits = vec![];
    let label;
    let input;
    if let Some(adjustments) = adjustments {
        if adjustments.is_empty() || adjustments.len() > 4096 {
            return Err("demand edits require between 1 and 4096 adjustments".into());
        }
        let mut expected = anchor.clone();
        crate::study::apply_network_edits(
            &mut expected,
            &decision_edits(&goal.decisions, &changes),
        )?;
        let canonical = |network| {
            crate::ir::serialize_module(&powerio::PioModule::new(
                powerio::PioValue::BalancedNetwork(network),
            ))
        };
        if canonical(expected)? != canonical(current.clone())? {
            return Err("selected state contains changes outside the goal's permitted interventions; revise the goal first".into());
        }
        label = if adjustments.len() == 1 {
            format!(
                "Bus {}: {:+} MW",
                adjustments[0].bus, adjustments[0].delta_mw
            )
        } else {
            format!("Demand edits at {} buses", adjustments.len())
        };
        for adjustment in adjustments {
            if !adjustment.delta_mw.is_finite() {
                return Err("demand increments must be finite".into());
            }
            let bus =
                crate::objective::source_element_id(&current, crate::Axis::Bus, &adjustment.bus)?;
            let index = goal
                .decisions
                .variables
                .iter()
                .position(|v| {
                    v.intervention == Intervention::ActiveDemand
                        && crate::objective::source_element_id(
                            &current,
                            crate::Axis::Bus,
                            &v.element,
                        )
                        .ok()
                            == Some(bus)
                })
                .ok_or("demand edit is outside the goal's permitted buses")?;
            changes[index] += adjustment.delta_mw;
        }
        if !goal.decisions.within_limits(&changes, 1e-7) {
            return Err("cumulative demand edits exceed the declared bounds, increments, budget or bus count".into());
        }
        edits = decision_edits(&goal.decisions, &changes);
        input = bundle.artifacts[&bundle.document.states[&goal.anchor_state].input]
            .text
            .clone();
    } else {
        let base = bundle
            .document
            .base_input
            .as_ref()
            .ok_or("original base case is unavailable in this Study")?;
        input = bundle.artifacts[base].text.clone();
        let base = input_network(&input)?;
        same_bus_identities(&base, &current)?;
        changes = network_changes(&anchor, &base, &goal.decisions)?;
        label = "Original network base case".into();
    }
    let permitted_total = goal.decisions.feasible(&changes, 1e-7);
    let mut solve_count = 0;
    let run = (|| {
        if cancelled() {
            return Err("cancelled".into());
        }
        solve_count += 1;
        let mut study = Study::new(&input, state.formulation)?;
        if !restore {
            if cancelled() {
                return Err("cancelled".into());
            }
            solve_count += 1;
            study.commit(&edits)?;
        }
        let value = study.objective_value(&goal.objective, &goal.decisions, &changes)?;
        ExactCandidate::capture(&study, changes.clone(), value)
    })();
    let mut record = ExperimentRecord {
        start_state: Some(state_id.into()),
        goal: Some(goal_id.into()),
        kind: ExperimentKind::Counterfactual,
        rationale,
        evidence: vec![],
        trials: vec![],
        result_states: vec![],
        assessed_recommendation: None,
        solve_count,
        termination: "completed".into(),
    };
    let mut candidate = None;
    match run {
        Ok(exact) => {
            let state = bundle.capture_state(&exact, Some(state_id.into()), label)?;
            let evidence = bundle.add_artifact(
                ArtifactKind::Evidence,
                serde_json::json!({
                    "operation": if restore { "restore_base" } else { "edit_demand" },
                    "exact_value": exact.value, "goal_constraints_satisfied": permitted_total,
                    "cumulative_demand_changes": demand_changes(bundle, &state)?,
                    "solve_count": solve_count,
                })
                .to_string(),
            )?;
            record.evidence.push(evidence);
            record.result_states.push(state.clone());
            record.termination = if restore {
                "base_case_ready"
            } else if permitted_total {
                "completed"
            } else {
                "demand_total_incomplete"
            }
            .into();
            bundle.inspect(&state)?;
            if restore || permitted_total {
                candidate = Some(state);
            }
        }
        Err(error) => record.termination = error,
    }
    let evidence = record.evidence.clone();
    let id = bundle.add_experiment(record)?;
    bundle.document.recommended_state = candidate.clone();
    if let Some(state) = candidate {
        bundle.add_decision(DecisionRecord {
            experiment: id.clone(), state: Some(state), choice: DecisionKind::Recommend,
            rationale: if restore { "Exact restoration of the retained network data, awaiting explicit application" }
                else { "Demand edits satisfy the declared intervention limits and total, awaiting explicit application" }.into(),
            evidence,
        })?;
    }
    Ok(id)
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
    use crate::NetworkEdit;
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
            base_input: None,
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

    fn demand_fixture(edited_start: bool) -> StudyBundle {
        demand_fixture_with_initial_load(edited_start, 20.0)
    }

    fn demand_fixture_with_initial_load(edited_start: bool, bus3_mw: f64) -> StudyBundle {
        let mut bundle = fixture();
        let root = bundle.document.applied_state.clone().unwrap();
        let input = bundle.artifacts[&bundle.document.states[&root].input]
            .text
            .clone();
        let mut network = input_network(&input).unwrap();
        for branch in network.branches_mut() {
            branch.rate_a = 1000.0;
        }
        if bus3_mw > 0.0 {
            crate::study::apply_network_edits(
                &mut network,
                &[NetworkEdit::AddLoad {
                    bus: 3.into(),
                    p_mw: bus3_mw,
                }],
            )
            .unwrap();
        }
        let input = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(network),
        ))
        .unwrap();
        let mut original = Study::new(&input, Problem::DcOpf).unwrap();
        let base_input = original.save_instance_module().unwrap();
        if edited_start {
            original
                .commit(&[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 3.0,
                }])
                .unwrap();
        }
        let mut goal = bundle.document.goals[&bundle.document.active_goal.clone().unwrap()].clone();
        goal.decisions = DecisionSpace {
            variables: [2, 3]
                .into_iter()
                .map(|bus| DecisionVariable {
                    id: format!("bus{bus}"),
                    element: bus.into(),
                    intervention: Intervention::ActiveDemand,
                    lower: -10.0,
                    upper: 10.0,
                    increment: 1.0,
                    budget_weight: 1.0,
                })
                .collect(),
            total_budget: 20.0,
            max_changed_elements: 2,
            demand: Some(crate::objective::DemandConstraint::Redistribution),
        };
        bundle = create_study(CreateStudy {
            id: "demand".into(),
            title: "Demand transfers".into(),
            input: original.save_instance_module().unwrap(),
            base_input: Some(base_input),
            formulation: Problem::DcOpf,
            request: "Move demand between buses 2 and 3".into(),
            interpretation: "Preserve total demand with at most 10 MW per bus".into(),
            objective: goal.objective,
            decisions: goal.decisions,
            success_value: None,
        })
        .unwrap();
        bundle
    }

    #[test]
    fn demand_can_be_added_repeatedly_at_a_bus_without_an_existing_load() {
        let mut bundle = demand_fixture_with_initial_load(false, 0.0);
        for (bus, delta_mw) in [
            (3, 2.0),
            (3, -2.0),
            (3, 2.0),
            (2, -2.0),
            (3, 1.0),
            (2, -1.0),
        ] {
            let result = edit_demand(&mut bundle, bus, delta_mw);
            let record = &bundle.document.experiments[&result.experiment.unwrap()];
            assert_eq!(record.result_states.len(), 1);
        }
        let state = bundle.document.inspected_state.as_deref().unwrap();
        let changes = demand_changes(&bundle, state).unwrap().unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|change| change.delta_mw)
                .collect::<Vec<_>>(),
            [-3.0, 3.0]
        );
    }

    fn edit_demand(bundle: &mut StudyBundle, bus: i64, delta_mw: f64) -> StudyOperationResult {
        let request = StudyRequest {
            expected_revision: bundle.document.revision,
            operation: StudyOperation::EditDemand {
                state: bundle.document.inspected_state.clone().unwrap(),
                goal: bundle.document.active_goal.clone().unwrap(),
                changes: vec![DemandAdjustment {
                    bus: bus.into(),
                    delta_mw,
                }],
                rationale: format!("Adjust bus {bus} demand by {delta_mw} MW"),
            },
        };
        execute_study(bundle, request, || false).unwrap()
    }

    #[test]
    fn demand_edits_accumulate_across_buses_reload_and_return_to_original_base() {
        let mut bundle = demand_fixture(true);
        let root = bundle.document.applied_state.clone().unwrap();
        let first = edit_demand(&mut bundle, 2, 2.0);
        assert_eq!(first.demand_changes.as_ref().unwrap()[0].delta_mw, 5.0);
        assert!(bundle.document.recommended_state.is_none());
        assert_eq!(
            bundle.document.experiments[&first.experiment.unwrap()].termination,
            "demand_total_incomplete"
        );
        bundle = StudyBundle::import(&bundle.export().unwrap()).unwrap();
        edit_demand(&mut bundle, 3, -2.0);
        edit_demand(&mut bundle, 2, 1.0);
        let last = edit_demand(&mut bundle, 3, -1.0);
        let changes = last.demand_changes.unwrap();
        assert_eq!(
            changes.iter().map(|c| c.delta_mw).collect::<Vec<_>>(),
            vec![6.0, -3.0]
        );
        let adjusted = bundle.document.inspected_state.clone().unwrap();
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&root));
        let saved_count = bundle.document.states.len();
        let request = StudyRequest {
            expected_revision: bundle.document.revision,
            operation: StudyOperation::RestoreBase {
                state: adjusted.clone(),
                goal: bundle.document.active_goal.clone().unwrap(),
                rationale: "Return to the original network data".into(),
            },
        };
        let restored = execute_study(&mut bundle, request, || false).unwrap();
        assert!(restored.demand_changes.unwrap().is_empty());
        assert_eq!(bundle.document.states.len(), saved_count + 1);
        assert!(bundle.document.states.contains_key(&adjusted));
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&root));
        let reset = bundle.document.recommended_state.clone().unwrap();
        assert_ne!(reset, root);
        let operation = StudyOperation::Apply {
            proposal: restored.experiment.unwrap(),
            state: reset.clone(),
            goal: bundle.document.active_goal.clone().unwrap(),
            base_state: adjusted,
        };
        let revision = bundle.document.revision;
        execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: revision,
                operation,
            },
            || false,
        )
        .unwrap();
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&reset));
        bundle.validate().unwrap();
    }

    #[test]
    fn cumulative_demand_bounds_unknown_buses_and_missing_base_are_atomic() {
        let mut bundle = demand_fixture(false);
        edit_demand(&mut bundle, 2, 8.0);
        for (bus, delta_mw) in [(2, 3.0), (1, 1.0), (2, f64::NAN)] {
            let before = bundle.export().unwrap();
            let operation = StudyOperation::EditDemand {
                state: bundle.document.inspected_state.clone().unwrap(),
                goal: bundle.document.active_goal.clone().unwrap(),
                changes: vec![DemandAdjustment {
                    bus: bus.into(),
                    delta_mw,
                }],
                rationale: "Probe rejected edit".into(),
            };
            let revision = bundle.document.revision;
            assert!(execute_study(
                &mut bundle,
                StudyRequest {
                    expected_revision: revision,
                    operation
                },
                || false
            )
            .is_err());
            assert_eq!(before, bundle.export().unwrap());
        }
        bundle.document.base_input = None;
        let before = bundle.export().unwrap();
        let operation = StudyOperation::RestoreBase {
            state: bundle.document.inspected_state.clone().unwrap(),
            goal: bundle.document.active_goal.clone().unwrap(),
            rationale: "Restore unavailable source".into(),
        };
        let revision = bundle.document.revision;
        assert!(execute_study(
            &mut bundle,
            StudyRequest {
                expected_revision: revision,
                operation
            },
            || false
        )
        .unwrap_err()
        .contains("unavailable"));
        assert_eq!(before, bundle.export().unwrap());
        assert!(StudyBundle::import(&before)
            .unwrap()
            .document
            .base_input
            .is_none());
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
