//! Portable, application-owned Study history and content-addressed evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::objective::{DecisionSpace, StudyObjective};
use crate::Problem;

pub const STUDY_VERSION: u32 = 1;
const MAX_RECORDS: usize = 100_000;
const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;

pub fn content_id(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn record_id<T: Serialize>(value: &T) -> Result<String, String> {
    // Value orders object keys consistently across Rust and browser producers.
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    Ok(content_id(
        &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
    ))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    PowerioIr,
    Evidence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyArtifact {
    pub kind: ArtifactKind,
    pub text: String,
}

/// An immutable electrical state. Inspection never implies application.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StateNode {
    pub parent: Option<String>,
    pub formulation: Problem,
    pub input: String,
    pub solution: String,
    pub view: String,
    pub label: String,
}

/// A goal revision retains its own anchor and interpretation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoalRevision {
    pub parent: Option<String>,
    pub anchor_state: String,
    pub request: String,
    pub interpretation: String,
    pub objective: StudyObjective,
    pub decisions: DecisionSpace,
    pub success_value: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExperimentKind {
    Inspection,
    Sensitivity,
    Planning,
    Counterfactual,
    Challenge,
    HistoricalImport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrialRecord {
    pub changes: Vec<f64>,
    pub predicted_value: Option<f64>,
    pub exact_value: Option<f64>,
    pub state: Option<String>,
    pub accepted: bool,
    pub failure: Option<String>,
    pub evidence: Vec<String>,
}

/// Stated rationales and evidence references describe observable agent actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExperimentRecord {
    /// Historical imports can lack recoverable electrical state and goal data.
    pub start_state: Option<String>,
    pub goal: Option<String>,
    pub kind: ExperimentKind,
    pub rationale: String,
    pub evidence: Vec<String>,
    pub trials: Vec<TrialRecord>,
    pub result_states: Vec<String>,
    pub assessed_recommendation: Option<String>,
    pub solve_count: usize,
    pub termination: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Retain,
    Reject,
    Recommend,
    Apply,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionRecord {
    pub experiment: String,
    pub state: Option<String>,
    pub choice: DecisionKind,
    pub rationale: String,
    pub evidence: Vec<String>,
}

/// A document contains references, while the bundle owns deduplicated artifacts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyDocument {
    pub schema: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub revision: u64,
    pub goals: BTreeMap<String, GoalRevision>,
    pub states: BTreeMap<String, StateNode>,
    pub experiments: BTreeMap<String, ExperimentRecord>,
    pub experiment_order: Vec<String>,
    pub decisions: BTreeMap<String, DecisionRecord>,
    pub active_goal: Option<String>,
    pub inspected_state: Option<String>,
    pub recommended_state: Option<String>,
    pub applied_state: Option<String>,
    /// Original network input, retained independently of the starting operating point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_input: Option<String>,
}

/// Approvals are intentionally absent from the portable document.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyBundle {
    pub document: StudyDocument,
    pub artifacts: BTreeMap<String, StudyArtifact>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudySummary {
    pub id: String,
    pub title: String,
    pub revision: u64,
    pub active_goal: Option<(String, GoalRevision)>,
    pub inspected_state: Option<String>,
    pub recommended_state: Option<String>,
    pub applied_state: Option<String>,
    pub state_count: usize,
    pub experiment_count: usize,
    pub recent_experiments: Vec<ExperimentSummary>,
    pub unavailable_historical_states: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExperimentSummary {
    pub id: String,
    pub kind: ExperimentKind,
    pub start_state: Option<String>,
    pub goal: Option<String>,
    pub rationale: String,
    pub solve_count: usize,
    pub trial_count: usize,
    pub result_states: Vec<String>,
    pub termination: String,
}

impl StudyBundle {
    pub fn empty(id: String, title: String) -> Result<Self, String> {
        if id.trim().is_empty() || id.len() > 256 || title.len() > 4096 {
            return Err("Study requires a bounded identity and title".into());
        }
        Ok(Self {
            document: StudyDocument {
                schema: "tellegen-study".into(),
                version: STUDY_VERSION,
                id,
                title,
                revision: 0,
                goals: BTreeMap::new(),
                states: BTreeMap::new(),
                experiments: BTreeMap::new(),
                experiment_order: Vec::new(),
                decisions: BTreeMap::new(),
                active_goal: None,
                inspected_state: None,
                recommended_state: None,
                applied_state: None,
                base_input: None,
            },
            artifacts: BTreeMap::new(),
        })
    }

    pub fn import(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BUNDLE_BYTES {
            return Err("Study bundle exceeds the 512 MiB import limit".into());
        }
        #[derive(Deserialize)]
        struct Header {
            schema: Option<String>,
        }
        let header: Header =
            serde_json::from_str(text).map_err(|e| format!("invalid Study bundle: {e}"))?;
        if header.schema.as_deref() == Some("tellegen.experiment-journal") {
            return Self::import_journal(
                content_id(text.as_bytes()),
                "Imported journal evidence".into(),
                text,
            );
        }
        let bundle: Self =
            serde_json::from_str(text).map_err(|e| format!("invalid Study bundle: {e}"))?;
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn export(&self) -> Result<String, String> {
        self.validate()?;
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if text.len() > MAX_BUNDLE_BYTES {
            return Err(
                "Study exceeds 512 MiB; export a branch or remove unneeded artifacts in a copy"
                    .into(),
            );
        }
        Ok(text)
    }

    pub fn check_revision(&self, expected: u64) -> Result<(), String> {
        if self.document.revision != expected {
            return Err(format!(
                "stale Study revision: expected {expected}, current {}",
                self.document.revision
            ));
        }
        Ok(())
    }

    /// Run a completed operation against a copy and publish it only after validation.
    pub fn transaction<T>(
        &mut self,
        expected: u64,
        operation: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        self.check_revision(expected)?;
        let mut next = self.clone();
        let result = operation(&mut next)?;
        next.document.revision = expected.checked_add(1).ok_or("Study revision exhausted")?;
        next.validate()?;
        *self = next;
        Ok(result)
    }

    pub fn add_artifact(&mut self, kind: ArtifactKind, text: String) -> Result<String, String> {
        let artifact = StudyArtifact { kind, text };
        validate_artifact(&artifact)?;
        let id = record_id(&artifact)?;
        self.artifacts.entry(id.clone()).or_insert(artifact);
        Ok(id)
    }

    pub fn add_state(&mut self, state: StateNode) -> Result<String, String> {
        let id = record_id(&state)?;
        self.document.states.entry(id.clone()).or_insert(state);
        Ok(id)
    }

    pub fn capture_state(
        &mut self,
        candidate: &crate::exploration::ExactCandidate,
        parent: Option<String>,
        label: String,
    ) -> Result<String, String> {
        let view: crate::SolveResponse =
            serde_json::from_str(&candidate.view).map_err(|e| e.to_string())?;
        let input = self.add_artifact(ArtifactKind::PowerioIr, candidate.input.clone())?;
        let solution = self.add_artifact(ArtifactKind::PowerioIr, candidate.solution.clone())?;
        let evidence = self.add_artifact(ArtifactKind::Evidence, candidate.view.clone())?;
        self.add_state(StateNode {
            parent,
            formulation: view.formulation,
            input,
            solution,
            view: evidence,
            label,
        })
    }

    pub fn state_view(&self, state: &str) -> Result<crate::SolveResponse, String> {
        let state = self
            .document
            .states
            .get(state)
            .ok_or("state is unavailable")?;
        serde_json::from_str(
            &self
                .artifacts
                .get(&state.view)
                .ok_or("state evidence is unavailable")?
                .text,
        )
        .map_err(|e| e.to_string())
    }

    /// Import completed journal records as evidence with explicitly unavailable states.
    pub fn import_journal(id: String, title: String, text: &str) -> Result<Self, String> {
        if text.len() > MAX_BUNDLE_BYTES {
            return Err("journal exceeds the import limit".into());
        }
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value["schema"] != "tellegen.experiment-journal"
            || value["version"] != 1
            || !value["records"].is_array()
        {
            return Err("unsupported historical journal".into());
        }
        let mut bundle = Self::empty(id, title)?;
        let artifact = bundle.add_artifact(ArtifactKind::Evidence, text.into())?;
        bundle.add_experiment(ExperimentRecord {
            start_state: None,
            goal: None,
            kind: ExperimentKind::HistoricalImport,
            rationale: "Imported completed journal evidence; electrical states are unavailable"
                .into(),
            evidence: vec![artifact],
            trials: vec![],
            result_states: vec![],
            assessed_recommendation: None,
            solve_count: 0,
            termination: "historical_evidence_only".into(),
        })?;
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn revise_goal(&mut self, goal: GoalRevision) -> Result<String, String> {
        let state = self
            .document
            .states
            .get(&goal.anchor_state)
            .ok_or("goal anchor state is unavailable")?;
        goal.decisions.validate(state.formulation)?;
        goal.objective.validate(&goal.decisions)?;
        let network = crate::study_ops::state_network(self, &goal.anchor_state)?;
        let view = self.state_view(&goal.anchor_state)?;
        let changes = goal
            .decisions
            .variables
            .iter()
            .map(|v| (v.id.clone(), 0.0))
            .collect();
        goal.objective.evaluate(&view, &network, &changes)?;
        if goal.parent != self.document.active_goal {
            return Err("goal revision must name the current goal as its parent".into());
        }
        let id = record_id(&goal)?;
        self.document.goals.insert(id.clone(), goal);
        self.document.active_goal = Some(id.clone());
        self.document.recommended_state = None;
        Ok(id)
    }

    pub fn inspect(&mut self, state: &str) -> Result<(), String> {
        if !self.document.states.contains_key(state) {
            return Err("inspection state is unavailable".into());
        }
        self.document.inspected_state = Some(state.into());
        Ok(())
    }

    pub fn add_experiment(&mut self, experiment: ExperimentRecord) -> Result<String, String> {
        let id = record_id(&(self.document.experiment_order.len(), &experiment))?;
        if !self.document.experiments.contains_key(&id) {
            self.document.experiment_order.push(id.clone());
        }
        self.document
            .experiments
            .entry(id.clone())
            .or_insert(experiment);
        Ok(id)
    }

    pub fn add_decision(&mut self, decision: DecisionRecord) -> Result<String, String> {
        let id = record_id(&decision)?;
        self.document
            .decisions
            .entry(id.clone())
            .or_insert(decision);
        Ok(id)
    }

    pub fn summary(&self, limit: usize) -> StudySummary {
        let d = &self.document;
        StudySummary {
            id: d.id.clone(),
            title: d.title.clone(),
            revision: d.revision,
            active_goal: d
                .active_goal
                .as_ref()
                .and_then(|id| d.goals.get(id).map(|g| (id.clone(), g.clone()))),
            inspected_state: d.inspected_state.clone(),
            recommended_state: d.recommended_state.clone(),
            applied_state: d.applied_state.clone(),
            state_count: d.states.len(),
            experiment_count: d.experiments.len(),
            recent_experiments: d
                .experiment_order
                .iter()
                .rev()
                .take(limit.min(20))
                .map(|id| {
                    let e = &d.experiments[id];
                    ExperimentSummary {
                        id: id.clone(),
                        kind: e.kind.clone(),
                        start_state: e.start_state.clone(),
                        goal: e.goal.clone(),
                        rationale: e.rationale.chars().take(320).collect(),
                        solve_count: e.solve_count,
                        trial_count: e.trials.len(),
                        result_states: e.result_states.iter().take(8).cloned().collect(),
                        termination: e.termination.clone(),
                    }
                })
                .collect(),
            unavailable_historical_states: d
                .experiments
                .values()
                .filter(|e| e.start_state.is_none())
                .count(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let d = &self.document;
        if d.schema != "tellegen-study" || d.version != STUDY_VERSION {
            return Err("unsupported Study document version".into());
        }
        if d.id.trim().is_empty() || d.id.len() > 256 || d.title.len() > 4096 {
            return Err("invalid Study identity or title".into());
        }
        if [
            d.states.len(),
            d.goals.len(),
            d.experiments.len(),
            d.decisions.len(),
            self.artifacts.len(),
        ]
        .into_iter()
        .sum::<usize>()
            > MAX_RECORDS
        {
            return Err("Study contains more than 100000 records".into());
        }
        if self.artifacts.values().map(|a| a.text.len()).sum::<usize>() > MAX_BUNDLE_BYTES {
            return Err("Study artifacts exceed 512 MiB".into());
        }
        let mut electrical = BTreeMap::new();
        for (id, a) in &self.artifacts {
            verify_record(id, a)?;
            if let Some(identity) = validate_artifact(a)? {
                electrical.insert(id, identity);
            }
        }
        if let Some(base) = &d.base_input {
            self.require_artifact(base, true)?;
            crate::study_ops::input_network(&self.artifacts[base].text)?;
        }
        for (id, s) in &d.states {
            verify_record(id, s)?;
            self.require_artifact(&s.input, true)?;
            self.require_artifact(&s.solution, true)?;
            self.require_artifact(&s.view, false)?;
            if let Some(parent) = &s.parent {
                require(&d.states, parent, "parent state")?;
            }
            if self.state_view(id)?.formulation != s.formulation {
                return Err("state view has a different formulation".into());
            }
            let input = electrical.get(&s.input);
            let solution = electrical.get(&s.solution);
            match (input, solution) {
                (Some((input_kind, false, input_id)), Some((solution_kind, true, solution_id)))
                    if *input_kind == s.formulation && *solution_kind == s.formulation =>
                {
                    if input_id != solution_id {
                        return Err("Study solution belongs to a different problem instance".into());
                    }
                }
                _ => {
                    return Err(
                        "Study state must pair its declared instance and solution formulation"
                            .into(),
                    )
                }
            }
        }
        for id in d.states.keys() {
            let mut visited = BTreeSet::new();
            let mut current = Some(id);
            while let Some(key) = current {
                if !visited.insert(key) {
                    return Err("Study states contain a cycle".into());
                }
                current = d.states[key].parent.as_ref();
            }
        }
        for (id, g) in &d.goals {
            verify_record(id, g)?;
            require(&d.states, &g.anchor_state, "goal anchor")?;
            g.decisions
                .validate(d.states[&g.anchor_state].formulation)?;
            g.objective.validate(&g.decisions)?;
            if g.success_value.is_some_and(|x| !x.is_finite()) {
                return Err("goal success value must be finite".into());
            }
            if let Some(parent) = &g.parent {
                require(&d.goals, parent, "parent goal")?;
            }
        }
        for id in d.goals.keys() {
            let mut visited = BTreeSet::new();
            let mut current = Some(id);
            while let Some(key) = current {
                if !visited.insert(key) {
                    return Err("Study goals contain a cycle".into());
                }
                current = d.goals[key].parent.as_ref();
            }
        }
        if d.experiment_order.len() != d.experiments.len()
            || d.experiment_order.iter().collect::<BTreeSet<_>>()
                != d.experiments.keys().collect::<BTreeSet<_>>()
        {
            return Err("experiment order must name every experiment exactly once".into());
        }
        for (position, id) in d.experiment_order.iter().enumerate() {
            let e = &d.experiments[id];
            verify_record(id, &(position, e))?;
            if !matches!(e.kind, ExperimentKind::HistoricalImport)
                && (e.start_state.is_none() || e.goal.is_none())
            {
                return Err("experiment requires a starting state and goal revision".into());
            }
            if let Some(state) = &e.start_state {
                require(&d.states, state, "experiment start")?;
            }
            if let Some(goal) = &e.goal {
                require(&d.goals, goal, "experiment goal")?;
            }
            for state in e
                .result_states
                .iter()
                .chain(e.assessed_recommendation.iter())
            {
                require(&d.states, state, "experiment result")?;
            }
            for evidence in &e.evidence {
                self.require_artifact(evidence, false)?;
            }
            if matches!(
                e.kind,
                ExperimentKind::Inspection | ExperimentKind::Sensitivity
            ) && (!e.result_states.is_empty() || e.trials.iter().any(|t| t.state.is_some()))
            {
                return Err(
                    "inspection and sensitivity experiments cannot create electrical states".into(),
                );
            }
            for trial in &e.trials {
                if trial.changes.iter().any(|x| !x.is_finite())
                    || trial.predicted_value.is_some_and(|x| !x.is_finite())
                    || trial.exact_value.is_some_and(|x| !x.is_finite())
                {
                    return Err("trial values must be finite".into());
                }
                if let Some(state) = &trial.state {
                    require(&d.states, state, "trial state")?;
                }
                if trial.accepted
                    && (trial.state.is_none()
                        || trial.exact_value.is_none()
                        || trial.failure.is_some())
                {
                    return Err("accepted trial requires an exact successful state".into());
                }
                for evidence in &trial.evidence {
                    self.require_artifact(evidence, false)?;
                }
            }
        }
        for (id, decision) in &d.decisions {
            verify_record(id, decision)?;
            require(&d.experiments, &decision.experiment, "decision experiment")?;
            if let Some(state) = &decision.state {
                require(&d.states, state, "decision state")?;
            }
            for evidence in &decision.evidence {
                self.require_artifact(evidence, false)?;
            }
        }
        for pointer in [&d.inspected_state, &d.recommended_state, &d.applied_state]
            .into_iter()
            .flatten()
        {
            require(&d.states, pointer, "state pointer")?;
        }
        if let Some(goal) = &d.active_goal {
            require(&d.goals, goal, "active goal")?;
        }
        Ok(())
    }

    fn require_artifact(&self, id: &str, electrical: bool) -> Result<(), String> {
        let a = self
            .artifacts
            .get(id)
            .ok_or_else(|| format!("missing Study artifact {id}"))?;
        if matches!(a.kind, ArtifactKind::PowerioIr) != electrical {
            return Err("Study artifact has an unexpected kind".into());
        }
        Ok(())
    }
}

fn verify_record<T: Serialize>(id: &str, record: &T) -> Result<(), String> {
    if id != record_id(record)? {
        return Err(format!("Study hash mismatch for {id}"));
    }
    Ok(())
}

fn require<T>(records: &BTreeMap<String, T>, id: &str, kind: &str) -> Result<(), String> {
    if !records.contains_key(id) {
        return Err(format!("missing {kind} {id}"));
    }
    Ok(())
}

/// Validate each electrical artifact once and retain only its instance identity.
/// Solution identities include the entire embedded instance, including costs and constraints.
fn validate_artifact(artifact: &StudyArtifact) -> Result<Option<(Problem, bool, String)>, String> {
    if artifact.text.len() > MAX_BUNDLE_BYTES {
        return Err("Study artifact exceeds 512 MiB".into());
    }
    if !matches!(artifact.kind, ArtifactKind::PowerioIr) {
        serde_json::from_str::<serde_json::Value>(&artifact.text)
            .map_err(|e| format!("invalid Study artifact: {e}"))?;
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct Header {
        schema: String,
        version: u32,
    }
    let header: Header =
        serde_json::from_str(&artifact.text).map_err(|e| format!("invalid Study artifact: {e}"))?;
    if header.schema != "pio-ir" || header.version != 2 {
        return Err("electrical Study artifacts require PowerIO IR generation 2".into());
    }
    let module = crate::ir::deserialize_module(&artifact.text)?;
    use powerio::PioValue as V;
    let (formulation, solution, instance) = match module.value() {
        V::DcOpfInstance(i) => (Problem::DcOpf, false, V::DcOpfInstance(i.clone())),
        V::AcPfInstance(i) => (Problem::AcPf, false, V::AcPfInstance(i.clone())),
        V::AcOpfInstance(i) => (Problem::Socwr, false, V::AcOpfInstance(i.clone())),
        V::DcOpfSolution(s) => (Problem::DcOpf, true, V::DcOpfInstance(s.instance().clone())),
        V::AcPfSolution(s) => (Problem::AcPf, true, V::AcPfInstance(s.instance().clone())),
        V::SocwrOpfSolution(s) => (Problem::Socwr, true, V::AcOpfInstance(s.instance().clone())),
        _ => return Ok(None),
    };
    let canonical = crate::ir::serialize_module(&powerio::PioModule::new(instance))?;
    Ok(Some((
        formulation,
        solution,
        content_id(canonical.as_bytes()),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objective::{DecisionVariable, Intervention, ObservableWeight};
    use crate::{NetworkEdit, Operand, Power, Study};

    fn fixture() -> (StudyBundle, Study, String) {
        let net = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        let input = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(net),
        ))
        .unwrap();
        let study = Study::new(&input, Problem::DcOpf).unwrap();
        let mut bundle = StudyBundle::empty("study-test".into(), "Regional prices".into()).unwrap();
        let exact = crate::exploration::ExactCandidate::capture(&study, vec![], 0.0).unwrap();
        let root = bundle
            .capture_state(&exact, None, "Starting point".into())
            .unwrap();
        bundle.document.inspected_state = Some(root.clone());
        bundle.document.applied_state = Some(root.clone());
        let goal = GoalRevision {
            parent: None,
            anchor_state: root.clone(),
            request: "Lower prices".into(),
            interpretation: "Minimize bus 2 LMP".into(),
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
                    upper: 20.0,
                    increment: 1.0,
                    budget_weight: 1.0,
                }],
                total_budget: 20.0,
                max_changed_elements: 1,
                demand: None,
            },
            success_value: None,
        };
        bundle.revise_goal(goal).unwrap();
        bundle.validate().unwrap();
        (bundle, study, root)
    }

    #[test]
    fn branches_revisions_reload_and_inspection_preserve_applied_state() {
        let (mut bundle, mut study, root) = fixture();
        study
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 1.into(),
                delta_mw: 5.0,
            }])
            .unwrap();
        let exact = crate::exploration::ExactCandidate::capture(&study, vec![5.0], 0.0).unwrap();
        let branch = bundle
            .transaction(0, |next| {
                let state = next.capture_state(&exact, Some(root.clone()), "Candidate".into())?;
                next.inspect(&state)?;
                next.document.recommended_state = Some(state.clone());
                Ok(state)
            })
            .unwrap();
        assert_eq!(bundle.document.applied_state.as_ref(), Some(&root));
        assert_eq!(bundle.document.inspected_state.as_ref(), Some(&branch));
        let old_goal = bundle.document.active_goal.clone().unwrap();
        let mut revised = bundle.document.goals[&old_goal].clone();
        revised.parent = Some(old_goal.clone());
        revised.request = "Different target".into();
        bundle
            .transaction(1, |next| next.revise_goal(revised))
            .unwrap();
        assert_eq!(bundle.document.goals.len(), 2);
        assert_eq!(bundle.document.goals[&old_goal].request, "Lower prices");
        assert!(bundle.document.recommended_state.is_none());
        let text = bundle.export().unwrap();
        let imported = StudyBundle::import(&text).unwrap();
        assert_eq!(imported.document.applied_state, Some(root));
        assert_eq!(
            imported.state_view(&branch).unwrap().formulation,
            Problem::DcOpf
        );
        assert!(!text.contains("approval"));
    }

    #[test]
    fn tampering_missing_references_and_failed_transactions_are_rejected() {
        let (mut bundle, _, root) = fixture();
        let before = bundle.export().unwrap();
        assert!(bundle.transaction(1, |_| Ok(())).is_err());
        assert!(bundle
            .transaction(0, |next| {
                next.document.inspected_state = Some("missing".into());
                Ok(())
            })
            .is_err());
        assert_eq!(bundle.export().unwrap(), before);
        let input = bundle.document.states[&root].input.clone();
        bundle.artifacts.get_mut(&input).unwrap().text.push(' ');
        assert!(bundle.validate().unwrap_err().contains("hash mismatch"));
        let mut value: serde_json::Value = serde_json::from_str(&before).unwrap();
        value["document"]["version"] = 999.into();
        assert!(StudyBundle::import(&value.to_string())
            .unwrap_err()
            .contains("version"));
        value["document"]["version"] = 1.into();
        value["document"]["approval"] = true.into();
        assert!(StudyBundle::import(&value.to_string()).is_err());
    }

    #[test]
    fn valid_artifact_hashes_cannot_pair_different_problem_instances() {
        let (mut bundle, mut study, root) = fixture();
        study
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 1.into(),
                delta_mw: 5.0,
            }])
            .unwrap();
        let exact = crate::exploration::ExactCandidate::capture(&study, vec![5.0], 0.0).unwrap();
        let candidate = bundle
            .capture_state(&exact, Some(root.clone()), "Candidate".into())
            .unwrap();
        let mut mismatched = bundle.document.states[&candidate].clone();
        mismatched.input = bundle.document.states[&root].input.clone();
        bundle.add_state(mismatched).unwrap();
        assert!(bundle
            .validate()
            .unwrap_err()
            .contains("different problem instance"));
    }

    #[test]
    fn historical_journal_import_does_not_invent_states() {
        let bundle = StudyBundle::import_journal("legacy".into(), "Imported evidence".into(),
            r#"{"schema":"tellegen.experiment-journal","version":1,"records":[{"toolName":"solve","input":{"command":"do not execute"}}]}"#).unwrap();
        assert!(bundle.document.states.is_empty());
        assert!(bundle.document.applied_state.is_none());
        assert_eq!(bundle.summary(5).unavailable_historical_states, 1);
        let imported = StudyBundle::import(
            r#"{"schema":"tellegen.experiment-journal","version":1,"records":[]}"#,
        )
        .unwrap();
        assert!(imported.document.states.is_empty());
        assert!(imported.document.applied_state.is_none());
        assert_eq!(
            bundle
                .document
                .experiments
                .values()
                .next()
                .unwrap()
                .solve_count,
            0
        );
    }

    #[test]
    fn exact_ac_and_socwr_artifacts_restore_the_declared_instance() {
        let net = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        let mut instance = powerio::AcPfInstance::from_network(net).unwrap();
        let mut specs = instance.specifications().to_vec();
        if let powerio::AcBusSpecification::Pv { p, .. } = &mut specs[2] {
            *p = 50.0;
        }
        instance = powerio::AcPfInstance::new(instance.network().clone(), specs).unwrap();
        let text = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::AcPfInstance(instance),
        ))
        .unwrap();
        let mut study = Study::new(&text, Problem::AcPf).unwrap();
        study
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 3.0,
            }])
            .unwrap();
        let exact = crate::exploration::ExactCandidate::capture(&study, vec![3.0], 0.0).unwrap();
        let mut bundle = StudyBundle::empty("ac".into(), "Voltage target".into()).unwrap();
        bundle.capture_state(&exact, None, "AC".into()).unwrap();
        bundle.validate().unwrap();
        let restored = Study::new(&exact.input, Problem::AcPf).unwrap();
        let old = study.solution().vm.as_ref().unwrap();
        let new = restored.solution().vm.as_ref().unwrap();
        for (a, b) in old.iter().zip(new) {
            assert!((a.value - b.value).abs() < 1e-9);
        }
        #[cfg(feature = "conic")]
        {
            let net = crate::model::parse_matpower(crate::model::CASE3).unwrap();
            let text = crate::ir::serialize_module(&powerio::PioModule::new(
                powerio::PioValue::BalancedNetwork(net),
            ))
            .unwrap();
            let study = Study::new(&text, Problem::Socwr).unwrap();
            let exact = crate::exploration::ExactCandidate::capture(&study, vec![], 0.0).unwrap();
            let mut bundle = StudyBundle::empty("soc".into(), "Relaxation".into()).unwrap();
            bundle.capture_state(&exact, None, "SOCWR".into()).unwrap();
            bundle.validate().unwrap();
            assert!(exact.solution.contains("powerio.SocwrOpfSolution"));
        }
    }
}
