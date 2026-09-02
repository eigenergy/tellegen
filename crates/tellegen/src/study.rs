//! A retained interactive solve over a PowerIO module.
//!
//! Parse and build the model once, then [`commit`](Study::commit) exactly solves
//! at the new operating point and [`preview`](Study::preview) returns a first-order
//! linearization at the committed point with no re-solve. Interactive callers can use
//! `preview` while an edit is in flight and `commit` when it is accepted.
//!
//! A `Study` is one live solver session: the base case plus an ordered edit log, re-solved
//! from the base at every commit. Saving writes the current materialized network as a
//! stored PowerIO module ([`save_module`](Study::save_module)) with one descriptive
//! history entry per committed edit; a reloaded module starts a fresh session.
//!
//! The formulation coupling is a single boxed `SolvedState` trait object: the same
//! `Differentiable` dispatch the module solver uses, captured once at construction
//! and at every commit. Every formulation the build includes is supported — DC OPF and
//! AC power flow always, and the SOCWR relaxation behind `conic` — and a formulation this
//! build omits returns a typed error.

use std::collections::{BTreeMap, HashMap};

use powerio::{BalancedNetwork, PioModule, PioValue};
#[cfg(feature = "conic")]
use powerio_prob::AcOpfInstance;
use powerio_prob::{AcPfInstance, DcOpfInstance};
use serde::{Deserialize, Serialize};

use crate::api::{
    ac_pf_assemble, ac_pf_solved, dc_opf_assemble, dc_opf_solved, run_cells,
    validate_canonical_edits, Edits, ElementKey, Problem, SensRequest, SolveRequest, SolveResponse,
};
use crate::model::{AcNetwork, DcNetwork};
use crate::problem::AcPfSolution;
use crate::problem::DcOpfSolution;
use crate::sens::{
    AcNewton, Axis, DcKkt, Differentiable, ElementId, Mode, Operand, Parameter, Power,
};

#[cfg(feature = "conic")]
use crate::api::{socwr_assemble, socwr_solved};
#[cfg(feature = "conic")]
use crate::problem::SocWrSolution;
#[cfg(feature = "conic")]
use crate::sens::ConicKkt;

/// A typed edit to the operating point: the continuous active-demand drag and the
/// branch thermal-rating drag. The enum is `#[non_exhaustive]` and serde-tagged
/// (`{"kind":"add_load","bus":2,"p_mw":50}` /
/// `{"kind":"adjust_branch_rating","branch":3,"delta_mw":-25}`), so topology and
/// other-parameter edits extend the wire format without breaking a client that knows
/// only the demand edit. The element key is an [`ElementKey`] — the original numeric
/// id, or the powerio row uid (`"bus":"buses:1"`) when the network carries uids —
/// so a numeric client's wire shape is unchanged.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum NetworkEdit {
    /// Add `p_mw` to the active demand at this bus. Repeated edits accumulate; the
    /// committed operating point is the base case plus the whole log.
    AddLoad { bus: ElementKey, p_mw: f64 },
    /// Add `delta_mw` to the thermal rating of this branch. Accumulates like
    /// `AddLoad`; the committed limit is the base rating plus the log.
    AdjustBranchRating { branch: ElementKey, delta_mw: f64 },
}

impl NetworkEdit {
    /// The edited element's key, on the axis [`parameter`](Self::parameter) names.
    /// The id and uid forms of the same element are distinct keys in a fold; they
    /// resolve to the same dense element when applied, so their steps accumulate.
    fn element_key(&self) -> &ElementKey {
        match self {
            NetworkEdit::AddLoad { bus, .. } => bus,
            NetworkEdit::AdjustBranchRating { branch, .. } => branch,
        }
    }
    /// The edit's step magnitude in MW along its parameter.
    fn magnitude_mw(&self) -> f64 {
        match self {
            NetworkEdit::AddLoad { p_mw, .. } => *p_mw,
            NetworkEdit::AdjustBranchRating { delta_mw, .. } => *delta_mw,
        }
    }

    /// The [`Parameter`] this edit perturbs, so [`preview`](Study::preview) differentiates
    /// the watched operands with respect to the right axis. The active-demand drag maps to
    /// `Demand(Active)`, the rating drag to `LineLimit`; new edit kinds add their own arm
    /// here.
    fn parameter(&self) -> Parameter {
        match self {
            NetworkEdit::AddLoad { .. } => Parameter::Demand(Power::Active),
            NetworkEdit::AdjustBranchRating { .. } => Parameter::LineLimit,
        }
    }
}

/// A first-order preview of an edit at the committed operating point: the predicted
/// change in each watched operand, the predicted objective change, and the
/// linearization caveat.
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    /// One predicted operand-delta column per watched operand, in request order.
    pub operands: Vec<PreviewColumn>,
    /// First-order change in the declared objective along the edit. `None` for power
    /// flow or a feasibility objective. For a demand edit this is the committed
    /// marginal objective value dotted with the demand step.
    pub objective_delta: Option<f64>,
    /// Always `true`: a continuous edit's preview is a local linearization, valid only
    /// until a binding constraint changes. [`commit`](Study::commit) is the truth.
    pub local_only: bool,
}

/// The predicted change in one operand across the elements it ranges over.
#[derive(Clone, Debug, Serialize)]
pub struct PreviewColumn {
    pub operand: Operand,
    pub values: Vec<PreviewValue>,
    /// Served unit label of the prediction (for example `objective_unit/MW` or `pu`).
    pub units: String,
}

/// One element's predicted operand change, keyed by its source element id.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PreviewValue {
    pub element: ElementId,
    pub index: usize,
    pub value: f64,
}

/// The preview callback [`with_system`](SolvedState::with_system) hands the freshly built
/// KKT to: it runs the watched cells against the borrowed `&dyn Differentiable` and returns
/// their predicted columns. Factored out so the trait and its impls name one type.
type PreviewFn<'a> = dyn FnMut(&dyn Differentiable) -> Result<Vec<PreviewColumn>, String> + 'a;

/// The committed solved state of one formulation, retained so [`commit`](Study::commit)
/// can re-assemble its response (with any watched sensitivity cell) and
/// [`preview`](Study::preview) can build its differentiable KKT — both without re-solving.
///
/// This is the *sole* formulation coupling in the study: a `Box<dyn SolvedState>` carries
/// the formulation's model + solution and the three operations the study needs over them.
/// Each implementor builds its KKT *on the stack* in [`with_system`](SolvedState::with_system)
/// — the same on-the-stack borrow trick `run_cells` uses — so the `&dyn Differentiable`
/// borrow never escapes the callback and no factorization is ever cached across commits.
trait SolvedState {
    /// Re-assemble this formulation's [`SolveResponse`] at the committed point, computing
    /// any sensitivity cells in `req.sensitivities` in the same pass (no second solve).
    fn assemble(&self, req: &SolveRequest) -> Result<SolveResponse, String>;

    /// Build this formulation's differentiable KKT on the stack and hand the borrow to
    /// `f`. The borrow lives only for the call; `f` runs the preview cells against it.
    fn with_system(&self, f: &mut PreviewFn<'_>) -> Result<Vec<PreviewColumn>, String>;

    /// The committed marginal objective value per MW, per dense bus, for the
    /// preview's objective delta. `None` for power flow formulations (no objective).
    fn lmp(&self) -> Option<Vec<f64>>;

    /// Whether balance and limit duals have the economic interpretation used by
    /// price sensitivities and objective previews.
    fn has_economic_objective(&self) -> bool {
        false
    }

    /// The committed flow limit shadow value per MW of rating,
    /// per dense branch, for the rating preview's objective delta: ∂objective/∂rating
    /// = −shadow. `None` when the formulation exposes no flow-limit duals; the preview
    /// then reports no objective delta rather than a partial one.
    fn line_shadow_prices(&self) -> Option<Vec<f64>> {
        None
    }

    /// The committed DC model, when this formulation is the DC OPF; `None`
    /// otherwise. The planning engine differentiates only through the DC
    /// KKT today.
    fn dc_exact(&self) -> Option<(&DcNetwork, &DcOpfSolution)> {
        None
    }
}

/// DC OPF committed state.
struct DcState {
    net: DcNetwork,
    sol: DcOpfSolution,
}

impl SolvedState for DcState {
    fn assemble(&self, req: &SolveRequest) -> Result<SolveResponse, String> {
        dc_opf_assemble(&self.net, &self.sol, req)
    }
    fn with_system(&self, f: &mut PreviewFn<'_>) -> Result<Vec<PreviewColumn>, String> {
        f(&DcKkt::new(&self.net, &self.sol))
    }
    fn lmp(&self) -> Option<Vec<f64>> {
        self.has_economic_objective()
            .then(|| self.sol.nodal_marginal_values(self.net.base_mva))
    }
    fn has_economic_objective(&self) -> bool {
        self.net.objective == powerio_matrix::PreparedObjective::NetworkGeneratorCost
    }
    fn line_shadow_prices(&self) -> Option<Vec<f64>> {
        // Both limit rows relax when the rating grows, so the shadow is the dual sum;
        // the raw duals are per-unit, and the served value divides by base (the same
        // scaling as the LMP).
        let base = self.net.base_mva;
        self.has_economic_objective().then(|| {
            self.sol
                .lam_ub
                .iter()
                .zip(&self.sol.lam_lb)
                .map(|(&ub, &lb)| (ub + lb) / base)
                .collect()
        })
    }
    fn dc_exact(&self) -> Option<(&DcNetwork, &DcOpfSolution)> {
        Some((&self.net, &self.sol))
    }
}

/// AC power flow committed state.
struct AcPfState {
    net: AcNetwork,
    sol: AcPfSolution,
}

impl SolvedState for AcPfState {
    fn assemble(&self, req: &SolveRequest) -> Result<SolveResponse, String> {
        ac_pf_assemble(&self.net, &self.sol, req)
    }
    fn with_system(&self, f: &mut PreviewFn<'_>) -> Result<Vec<PreviewColumn>, String> {
        f(&AcNewton::new(&self.net, &self.sol))
    }
    fn lmp(&self) -> Option<Vec<f64>> {
        None
    }
}

/// SOCWR conic relaxation committed state.
#[cfg(feature = "conic")]
struct ConicState {
    net: AcNetwork,
    sol: SocWrSolution,
}

#[cfg(feature = "conic")]
impl SolvedState for ConicState {
    fn assemble(&self, req: &SolveRequest) -> Result<SolveResponse, String> {
        socwr_assemble(&self.net, &self.sol, req)
    }
    fn with_system(&self, f: &mut PreviewFn<'_>) -> Result<Vec<PreviewColumn>, String> {
        let sys = ConicKkt::new(&self.net, &self.sol).map_err(|e| e.to_string())?;
        f(&sys)
    }
    fn lmp(&self) -> Option<Vec<f64>> {
        let base = self.net.base_mva;
        self.has_economic_objective()
            .then(|| self.sol.lmp.iter().map(|v| v / base).collect())
    }
    fn has_economic_objective(&self) -> bool {
        self.net.objective == powerio_matrix::PreparedObjective::NetworkGeneratorCost
    }
}

/// Solve `req`'s formulation at `base + edits` from the retained PowerIO input and box the
/// committed state. The boxed [`SolvedState`] is the only formulation `match` the study
/// performs.
fn solve_state(
    input: &StudyInput,
    net: &BalancedNetwork,
    req: &SolveRequest,
) -> Result<Box<dyn SolvedState>, String> {
    match (input, req.formulation) {
        (StudyInput::BalancedNetwork, Problem::DcOpf) => {
            let instance = DcOpfInstance::from_network(net.clone()).map_err(|e| e.to_string())?;
            let (net, sol) = dc_opf_solved(DcNetwork::from_instance(&instance)?, req, None)?;
            Ok(Box::new(DcState { net, sol }))
        }
        (StudyInput::DcOpf(instance), Problem::DcOpf) => {
            let (net, sol) = dc_opf_solved(DcNetwork::from_instance(instance)?, req, None)?;
            Ok(Box::new(DcState { net, sol }))
        }
        (StudyInput::BalancedNetwork, Problem::AcPf) => {
            let instance = AcPfInstance::from_network(net.clone()).map_err(|e| e.to_string())?;
            let (net, sol) = ac_pf_solved(AcNetwork::from_pf_instance(&instance)?, req)?;
            Ok(Box::new(AcPfState { net, sol }))
        }
        (StudyInput::AcPf(instance), Problem::AcPf) => {
            let (net, sol) = ac_pf_solved(AcNetwork::from_pf_instance(instance)?, req)?;
            Ok(Box::new(AcPfState { net, sol }))
        }
        #[cfg(feature = "conic")]
        (StudyInput::BalancedNetwork, Problem::Socwr) => {
            let instance = AcOpfInstance::from_network(net.clone()).map_err(|e| e.to_string())?;
            let (net, sol) = socwr_solved(AcNetwork::from_instance(&instance)?, req)?;
            Ok(Box::new(ConicState { net, sol }))
        }
        #[cfg(feature = "conic")]
        (StudyInput::AcOpf(instance), Problem::Socwr) => {
            let (net, sol) = socwr_solved(AcNetwork::from_instance(instance)?, req)?;
            Ok(Box::new(ConicState { net, sol }))
        }
        (input, formulation) => Err(format!(
            "PowerIO {} input cannot start a {formulation:?} Study",
            input.kind()
        )),
    }
}

#[derive(Clone)]
enum StudyInput {
    BalancedNetwork,
    DcOpf(DcOpfInstance),
    AcPf(AcPfInstance),
    #[cfg(feature = "conic")]
    AcOpf(AcOpfInstance),
}

impl StudyInput {
    fn kind(&self) -> &'static str {
        match self {
            StudyInput::BalancedNetwork => "balanced_network",
            StudyInput::DcOpf(_) => "dc_opf_instance",
            StudyInput::AcPf(_) => "ac_pf_instance",
            #[cfg(feature = "conic")]
            StudyInput::AcOpf(_) => "ac_opf_instance",
        }
    }
}

/// A stateful, build-once handle. Construct with a network and a formulation; the base
/// network is retained as the source of truth, and the base case is solved immediately,
/// so [`solution`](Study::solution) and [`preview`](Study::preview) are available right
/// away. Every commit re-solves from a fresh clone of that base, so the operating point
/// is always `base + the whole edit log` — never an accumulated drift.
pub struct Study {
    formulation: Problem,
    input: StudyInput,
    /// The parsed base network: the source of truth re-solved (cloned) at every commit.
    base: BalancedNetwork,
    /// The stored source module without its retained byte owner. Re-reading it
    /// lets a save replace only the value while carrying the producer, source
    /// descriptors, diagnostics, history, and extensions. The save severs the
    /// obsolete source map and diagnostic targets after replacing the value.
    base_module_json: String,
    log: Vec<NetworkEdit>,
    /// Commit boundaries partitioning `log` into the batches each [`commit`](Study::commit)
    /// call appended. Each entry is a running prefix length into `log`, one per commit,
    /// non-decreasing, with `commit_bounds.last() == log.len()`. A study with no edits
    /// has no commits.
    commit_bounds: Vec<usize>,
    /// The committed solved state, the sole formulation coupling.
    solved: Box<dyn SolvedState>,
    last: SolveResponse,
}

impl Study {
    /// Read a retained PowerIO module and solve its declared network or problem
    /// instance. Bare model JSON is not a Study input.
    pub fn new(module_json: &str, formulation: Problem) -> Result<Self, String> {
        let module = crate::ir::deserialize_module(module_json).map_err(|e| e.to_string())?;
        Self::from_dynamic_module(module, formulation)
    }

    /// Build a retained module from a network for crate tests. Public callers
    /// use [`Study::new`] or [`Study::from_module`].
    #[cfg(test)]
    fn from_network(net: &BalancedNetwork, formulation: Problem) -> Result<Self, String> {
        let producer = powerio::Producer::new("tellegen", env!("CARGO_PKG_VERSION"))
            .map_err(|e| e.to_string())?;
        let module = PioModule::new(PioValue::BalancedNetwork(net.clone())).with_producer(producer);
        Self::from_dynamic_module(module, formulation)
    }

    /// Build from a typed PowerIO module, retaining all module records as the
    /// basis for a later materialized save.
    pub fn from_module(
        module: PioModule<BalancedNetwork>,
        formulation: Problem,
    ) -> Result<Self, String> {
        let net = module.value.clone();
        let dynamic = module.map_value(PioValue::BalancedNetwork);
        let module_json = crate::ir::serialize_module(&dynamic).map_err(|e| e.to_string())?;
        Self::from_network_and_module(&net, StudyInput::BalancedNetwork, formulation, module_json)
    }

    fn from_dynamic_module(
        module: PioModule<PioValue>,
        formulation: Problem,
    ) -> Result<Self, String> {
        let module_json = crate::ir::serialize_module(&module).map_err(|e| e.to_string())?;
        match module.into_value() {
            PioValue::BalancedNetwork(net) => Self::from_network_and_module(
                &net,
                StudyInput::BalancedNetwork,
                formulation,
                module_json,
            ),
            PioValue::DcOpfInstance(instance) => {
                let net = instance.network().clone();
                Self::from_network_and_module(
                    &net,
                    StudyInput::DcOpf(instance),
                    formulation,
                    module_json,
                )
            }
            PioValue::AcPfInstance(instance) => {
                let net = instance.network().clone();
                Self::from_network_and_module(
                    &net,
                    StudyInput::AcPf(instance),
                    formulation,
                    module_json,
                )
            }
            #[cfg(feature = "conic")]
            PioValue::AcOpfInstance(instance) => {
                let net = instance.network().clone();
                Self::from_network_and_module(
                    &net,
                    StudyInput::AcOpf(instance),
                    formulation,
                    module_json,
                )
            }
            other => Err(format!(
                "PowerIO module holds {}, which cannot start a Study",
                other.type_name()
            )),
        }
    }

    fn from_network_and_module(
        net: &BalancedNetwork,
        input: StudyInput,
        formulation: Problem,
        base_module_json: String,
    ) -> Result<Self, String> {
        // A base study must be safe before the first edit. Otherwise an empty
        // study could solve successfully and only discover ambiguous uid lookup
        // when its first preview, commit, save, or export is attempted.
        validate_canonical_edits(net, &Edits::default())?;
        let req = SolveRequest {
            formulation,
            edits: Edits::default(),
            ..Default::default()
        };
        let solved = solve_state(&input, net, &req)?;
        let last = solved.assemble(&req)?;
        Ok(Study {
            formulation,
            input,
            base: net.clone(),
            base_module_json,
            log: Vec::new(),
            commit_bounds: Vec::new(),
            solved,
            last,
        })
    }

    /// The formulation this study solves.
    pub fn formulation(&self) -> Problem {
        self.formulation
    }

    /// The most recent committed solution.
    pub fn solution(&self) -> &SolveResponse {
        &self.last
    }

    /// The committed edit log (the study): base case + these edits = the current point.
    pub fn edits(&self) -> &[NetworkEdit] {
        &self.log
    }

    /// Apply `edits` to the committed operating point and exact-re-solve, with no
    /// sensitivity cells. The zero-sensitivity convenience over
    /// [`commit_with`](Study::commit_with).
    pub fn commit(&mut self, edits: &[NetworkEdit]) -> Result<SolveResponse, String> {
        self.commit_with(edits, &[])
    }

    /// Apply `edits` to the committed operating point and exact-re-solve, attaching the
    /// requested `sensitivities` to the response in the **same** solve. This is the
    /// source of truth; the new solution becomes the committed point. The base network is
    /// reused (cloned and perturbed), so this never re-parses, and the watched cell rides
    /// back on `SolveResponse.sensitivities` with no second solve.
    pub fn commit_with(
        &mut self,
        edits: &[NetworkEdit],
        sensitivities: &[SensRequest],
    ) -> Result<SolveResponse, String> {
        let mut next_log = self.log.clone();
        next_log.extend_from_slice(edits);
        // One bound per call, so the log keeps this batch as its own commit
        // instead of folding it into the history.
        let mut next_bounds = self.commit_bounds.clone();
        next_bounds.push(next_log.len());
        self.commit_log(next_log, next_bounds, sensitivities)
    }

    /// Replace the committed edit set with `edits` and exact-re-solve, with no
    /// sensitivity cells. Use this when the caller owns an absolute state such as
    /// `base demand + deltas`, rather than an append-only edit log.
    pub fn replace_edits(&mut self, edits: &[NetworkEdit]) -> Result<SolveResponse, String> {
        self.replace_edits_with(edits, &[])
    }

    /// Replace the committed edit set with `edits` and exact-re-solve, attaching the
    /// requested `sensitivities` to the response in the same solve.
    ///
    /// This is the absolute state companion to [`commit_with`](Study::commit_with):
    /// `commit_with` appends new edits to the study log; `replace_edits_with` treats
    /// the supplied edits as the whole current operating point.
    pub fn replace_edits_with(
        &mut self,
        edits: &[NetworkEdit],
        sensitivities: &[SensRequest],
    ) -> Result<SolveResponse, String> {
        // Replacing the operating point discards the append history: the whole absolute
        // state becomes one commit (or none, when reset to base).
        let bounds = if edits.is_empty() {
            Vec::new()
        } else {
            vec![edits.len()]
        };
        self.commit_log(edits.to_vec(), bounds, sensitivities)
    }

    fn commit_log(
        &mut self,
        log: Vec<NetworkEdit>,
        commit_bounds: Vec<usize>,
        sensitivities: &[SensRequest],
    ) -> Result<SolveResponse, String> {
        validate_network_edits(&self.base, &log)?;
        let (solved, resp) = self.solve_log(&log, sensitivities)?;
        self.log = log;
        self.commit_bounds = commit_bounds;
        self.solved = solved;
        self.last = resp.clone();
        Ok(resp)
    }

    fn solve_log(
        &self,
        log: &[NetworkEdit],
        sensitivities: &[SensRequest],
    ) -> Result<(Box<dyn SolvedState>, SolveResponse), String> {
        let req = SolveRequest {
            formulation: self.formulation,
            edits: fold(log),
            sensitivities: sensitivities.to_vec(),
        };
        // Re-solve from a fresh clone of the base (the source of truth), then assemble the
        // response — including the requested sensitivity cells — from the committed state.
        let solved = solve_state(&self.input, &self.base, &req)?;
        let resp = solved.assemble(&req)?;
        Ok((solved, resp))
    }

    /// First-order prediction of applying `edits` at the committed point, for each
    /// `watched` operand, without re-solving. Builds the committed state's differentiable
    /// system **fresh** (never a cached factorization) and dots its `dz/dp` column with
    /// the edit step. The result is a local linearization (`local_only = true`); `commit`
    /// to confirm. Linearizes at the *last committed* state, not the base.
    pub fn preview(&self, edits: &[NetworkEdit], watched: &[Operand]) -> Result<Preview, String> {
        validate_network_edits(&self.base, edits)?;
        if !self.solved.has_economic_objective()
            && watched
                .iter()
                .any(|operand| matches!(operand, Operand::Price(_)))
        {
            return Err("price preview requires a network_generator_cost objective".to_owned());
        }
        // Group step magnitudes by the parameter each edit perturbs, keyed by the edited
        // element's key. A mixed edit set previews as the sum of the groups'
        // first-order terms (the linearization is additive across parameters). Groups
        // keep first-appearance order so errors and results are a function of the
        // request alone.
        let mut groups: Vec<(Parameter, HashMap<ElementKey, f64>)> = Vec::new();
        for e in edits {
            let p = e.parameter();
            let group = match groups.iter_mut().find(|(gp, _)| *gp == p) {
                Some((_, g)) => g,
                None => {
                    groups.push((p, HashMap::new()));
                    &mut groups.last_mut().expect("just pushed").1
                }
            };
            *group.entry(e.element_key().clone()).or_insert(0.0) += e.magnitude_mw();
        }

        // Dense element ids and uids per parameter axis, from the committed response's
        // ordering (the same order the sensitivity matrix reports).
        let bus_axis = response_bus_axis(&self.last);
        let branch_axis = response_branch_axis(&self.last);

        // An empty edit set previews as a zero demand step, preserving the demand-only
        // behavior (zero columns; objective delta 0 for OPF, None for power flow).
        if groups.is_empty() {
            groups.push((Parameter::Demand(Power::Active), HashMap::new()));
        }

        let resolved: Vec<(Parameter, Vec<usize>, Vec<f64>)> = groups
            .iter()
            .map(|(p, mag)| {
                let axis = match p.axis() {
                    Axis::Branch => &branch_axis,
                    _ => &bus_axis,
                };
                let (cols, col_mag) = dense_cols(axis, mag);
                (*p, cols, col_mag)
            })
            .collect();

        // Run each group's cells against one freshly built system, summing the per-operand
        // predictions elementwise: rows range over the operand axis, identical across
        // groups, so the merge is a plain vector add.
        let operands = self.solved.with_system(&mut |sys| {
            let mut merged: Option<Vec<PreviewColumn>> = None;
            for (parameter, cols, col_mag) in &resolved {
                let cols = preview_columns(sys, *parameter, cols, col_mag, watched)?;
                merged = Some(match merged.take() {
                    None => cols,
                    Some(acc) => merge_preview_columns(acc, cols)?,
                });
            }
            Ok(merged.unwrap_or_default())
        })?;

        // First-order objective change, summed across groups: balance marginal times
        // demand step, or negative limit marginal times a rating increase.
        // `None` when any group's dual vector is unavailable (power flow, or a formulation
        // without line shadow prices) — a partial sum would misreport the prediction.
        let mut objective_delta = Some(0.0);
        for (parameter, cols, col_mag) in &resolved {
            let contribution = match parameter {
                Parameter::LineLimit => self.solved.line_shadow_prices().map(|mu| {
                    -cols
                        .iter()
                        .zip(col_mag)
                        .map(|(&i, &m)| mu[i] * m)
                        .sum::<f64>()
                }),
                _ => self.solved.lmp().map(|lmp| {
                    cols.iter()
                        .zip(col_mag)
                        .map(|(&i, &m)| lmp[i] * m)
                        .sum::<f64>()
                }),
            };
            objective_delta = match (objective_delta, contribution) {
                (Some(acc), Some(x)) => Some(acc + x),
                _ => None,
            };
        }

        Ok(Preview {
            operands,
            objective_delta,
            local_only: true,
        })
    }

    /// First-order prediction for replacing the committed edit set with `target`.
    ///
    /// This accepts the same absolute edit state as [`replace_edits`](Study::replace_edits)
    /// while preserving [`preview`](Study::preview)'s semantics internally: it computes
    /// the incremental step from the current committed edits to `target`, then previews
    /// that step at the committed point.
    pub fn preview_replacement(
        &self,
        target: &[NetworkEdit],
        watched: &[Operand],
    ) -> Result<Preview, String> {
        validate_network_edits(&self.base, target)?;
        let step = replacement_step(&self.log, target);
        self.preview(&step, watched)
    }

    /// Run a bounded differentiable planning search over the committed
    /// operating point: the implicit gradient of the outer objective through
    /// the exact DC OPF KKT orders capacity trials whose every step is
    /// verified by an exact re-solve. Read only — the committed case,
    /// solution, edit log, and revision are untouched; the returned outcome
    /// carries the unapplied proposal and recorded search trace.
    pub fn plan(
        &self,
        spec: &crate::plan::CapacityPlanSpec,
    ) -> Result<crate::plan::CapacityPlanOutcome, String> {
        let Some((dc, solution)) = self.solved.dc_exact() else {
            return Err(format!(
                "planning requires the DC OPF formulation; this study solves {:?}",
                self.formulation
            ));
        };
        crate::plan::plan_capacity_from_exact(dc, solution, spec)
    }

    /// The number of committed edit batches. A study with no edits has zero.
    pub fn commits(&self) -> usize {
        self.commit_bounds.len()
    }

    /// Apply a geographic layer onto the base network: matched bus points land in
    /// `Bus.location`, matched routes in `Branch.route`. Locations are
    /// coordinate metadata the model never reads, so nothing re-solves; this keeps
    /// a live study's saved module consistent with the coordinates on screen
    /// (a save after this call carries them).
    pub fn apply_geo_layer(
        &mut self,
        layer: &powerio::geo::GeoLayer,
    ) -> powerio::geo::GeoApplyReport {
        self.base.apply_geo_layer(layer)
    }

    /// The current operating point as a network: a fresh clone of the base
    /// with the whole committed edit log applied.
    pub fn materialized_network(&self) -> Result<BalancedNetwork, String> {
        let mut net = self.base.clone();
        apply_network_edits(&mut net, &self.log)?;
        Ok(net)
    }

    /// Serialize the current operating point as a stored PowerIO module — the
    /// `powerio.module` document every PowerIO reader understands, with no
    /// tellegen format around it. The value is the materialized balanced
    /// network; each committed edit rides along as one descriptive `Edit`
    /// history entry created by the internal `history::edit_entry` helper.
    /// History describes how the value came to be and is never interpreted as state: a
    /// reader gets the correct network without it, and reloading the module
    /// starts a fresh session at this operating point.
    pub fn save_module(&self) -> Result<String, String> {
        crate::validate_canonical_identity(&self.base)?;
        let net = self.materialized_network()?;
        let module = self.retained_module(PioValue::BalancedNetwork(net))?;
        crate::ir::serialize_module(&module).map_err(|e| e.to_string())
    }

    /// Serialize the committed exact DC OPF result as a PowerIO solution
    /// module. Its embedded instance contains the materialized network that
    /// was solved.
    pub fn save_solution_module(&self) -> Result<String, String> {
        let Some((model, solution)) = self.solved.dc_exact() else {
            return Err(
                "an exact PowerIO solution module is available only for a DC OPF Study".to_owned(),
            );
        };
        let network = self.materialized_network()?;
        let instance = match &self.input {
            StudyInput::BalancedNetwork => {
                DcOpfInstance::from_network(network).map_err(|e| e.to_string())?
            }
            StudyInput::DcOpf(instance) => instance
                .clone()
                .with_network(network)
                .map_err(|e| e.to_string())?,
            _ => {
                return Err(
                    "the retained PowerIO input is not a DC OPF problem instance".to_owned(),
                );
            }
        };
        let exact = crate::emit::emit_dc_opf_solution(
            std::sync::Arc::new(instance),
            model,
            solution,
            format!("tellegen {}", env!("CARGO_PKG_VERSION")),
        )?;
        let module = self.retained_module(PioValue::DcOpfSolution(exact))?;
        crate::ir::serialize_module(&module).map_err(|e| e.to_string())
    }

    fn retained_module(&self, value: PioValue) -> Result<PioModule<PioValue>, String> {
        let source_module =
            crate::ir::deserialize_module(&self.base_module_json).map_err(|e| e.to_string())?;
        let mut module = source_module.map_value(|_| value).sever_source();
        module.sever_value_targets();
        let mut used_ids: std::collections::BTreeSet<String> = module
            .history()
            .iter()
            .map(|entry| entry.id().as_str().to_owned())
            .collect();
        let mut next_id = module.history().len();
        for edit in &self.log {
            while used_ids.contains(&format!("tellegen-edit-{next_id}")) {
                next_id += 1;
            }
            let entry = crate::history::edit_entry(next_id, edit)?;
            used_ids.insert(entry.id().as_str().to_owned());
            module.add_history_entry(entry).map_err(|e| e.to_string())?;
            next_id += 1;
        }
        Ok(module)
    }

    /// Export the committed operating point to `format` (`matpower`, `psse`,
    /// `model-json`, ...). The returned diagnostics belong to this output
    /// operation. Parse diagnostics remain on the retained PowerIO module.
    pub fn export(&self, format: &str) -> Result<ExportedCase, String> {
        let balanced = self.materialized_network()?;
        if format.eq_ignore_ascii_case("model-json") {
            let (text, diagnostics) = balanced
                .to_json_with_diagnostics()
                .map_err(|e| e.to_string())?;
            return Ok(ExportedCase {
                text,
                diagnostics,
                format: "model-json".to_owned(),
                extension: "json".to_owned(),
            });
        }

        let module = self.retained_module(PioValue::BalancedNetwork(balanced))?;
        let format_info = powerio::resolve_format(format);
        let destination_name = format_info.and_then(|info| info.extension).map_or_else(
            || "case".to_owned(),
            |extension| format!("case.{extension}"),
        );
        let emitted = powerio::emit(
            &module,
            format,
            powerio::Destination::memory(destination_name).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let diagnostics = emitted.diagnostics().to_vec();
        let powerio::EmittedOutput::Memory { mut artifacts } = emitted.into_output() else {
            return Err("PowerIO returned a path output for a memory destination".to_owned());
        };
        if artifacts.len() != 1 {
            return Err(format!(
                "PowerIO returned {} artifacts for a one file export",
                artifacts.len()
            ));
        }
        let artifact = artifacts.pop().expect("length checked");
        let artifact_name = artifact
            .name()
            .as_str()
            .rsplit('/')
            .next()
            .expect("artifact names are nonempty");
        let extension = artifact_name
            .strip_prefix("case.")
            .ok_or_else(|| {
                format!("PowerIO returned the unexpected artifact name {artifact_name:?}")
            })?
            .to_owned();
        let text = String::from_utf8(artifact.into_bytes())
            .map_err(|_| "PowerIO returned a non-UTF-8 text export".to_owned())?;
        let normalized_format = format_info
            .map(|info| info.token.to_owned())
            .unwrap_or_else(|| format.to_ascii_lowercase());
        Ok(ExportedCase {
            diagnostics,
            text,
            format: normalized_format,
            extension,
        })
    }
}

/// Validate every stateful edit against the canonical PowerIO payload before
/// commit or preview. Folding preserves zero-magnitude keys, so even a no-op
/// edit cannot use a display-only lowered row as a public target.
fn validate_network_edits(net: &BalancedNetwork, edits: &[NetworkEdit]) -> Result<(), String> {
    validate_canonical_edits(net, &fold(edits))
}

/// The numeric bus id for a bus edit key. A numeric key is the id itself; a uid
/// resolves to its row's `id` field. Errors on an unresolved uid.
fn bus_id_for_key(net: &BalancedNetwork, key: &ElementKey) -> Result<i64, String> {
    match key {
        ElementKey::Id(id) => Ok(*id),
        ElementKey::Uid(uid) => net
            .buses()
            .iter()
            .find(|b| b.uid.as_deref() == Some(uid))
            .map(|b| b.id.0 as i64)
            .ok_or_else(|| {
                format!("demand edit names bus uid \"{uid}\", which is not in the network")
            }),
    }
}

/// The 1-based branch position for a branch edit key. A numeric key is the position
/// itself; a uid resolves to its row index + 1. Errors on an unresolved uid.
fn branch_id_for_key(net: &BalancedNetwork, key: &ElementKey) -> Result<i64, String> {
    match key {
        ElementKey::Id(id) => Ok(*id),
        ElementKey::Uid(uid) => net
            .branches()
            .iter()
            .position(|br| br.uid.as_deref() == Some(uid))
            .map(|pos| (pos + 1) as i64)
            .ok_or_else(|| {
                format!("rating edit names branch uid \"{uid}\", which is not in the network")
            }),
    }
}

/// Materialize one edit batch onto the exported network, mirroring the
/// solver's semantics on the dense model: a demand delta lands on the bus's
/// first in service load (a new load appears when the bus has none), and a
/// rating delta moves a stated `rate_a`. Both fail closed: a negative bus
/// demand, a non positive resulting rating, an edit on a limit the source
/// never stated (`rate_a == 0` means unlimited), or a key that resolves to
/// no element rejects the export rather than writing a case that disagrees
/// with what was solved.
pub fn apply_network_edits(net: &mut BalancedNetwork, edits: &[NetworkEdit]) -> Result<(), String> {
    for edit in edits {
        match edit {
            NetworkEdit::AddLoad { bus, p_mw } => {
                let bus_id = bus_id_for_key(net, bus)?;
                let id = usize::try_from(bus_id)
                    .map_err(|_| format!("demand edit bus id {bus_id} out of range"))?;
                if !net.buses().iter().any(|b| b.id.0 == id) {
                    return Err(format!(
                        "demand edit names bus {bus}, which is not in the network"
                    ));
                }
                let total: f64 = net
                    .loads()
                    .iter()
                    .filter(|load| load.in_service && load.bus.0 == id)
                    .map(|load| load.p)
                    .sum();
                if total + p_mw < -1e-9 {
                    return Err(format!(
                        "demand delta for bus {bus} would make demand negative"
                    ));
                }
                match net
                    .loads_mut()
                    .iter_mut()
                    .find(|load| load.in_service && load.bus.0 == id)
                {
                    Some(load) => load.p += p_mw,
                    None => {
                        net.loads_mut()
                            .push(powerio::Load::new(powerio::BusId(id), *p_mw, 0.0))
                    }
                }
            }
            NetworkEdit::AdjustBranchRating { branch, delta_mw } => {
                let position = branch_id_for_key(net, branch)?;
                let row = usize::try_from(position)
                    .ok()
                    .and_then(|position| position.checked_sub(1))
                    .ok_or_else(|| format!("rating edit branch {branch} out of range"))?;
                let Some(edited) = net.branches_mut().get_mut(row) else {
                    return Err(format!(
                        "rating edit names branch {branch}, which is not in the network"
                    ));
                };
                if edited.rate_a <= 0.0 {
                    return Err(format!(
                        "rating edit on branch {branch} adjusts a limit the source does not \
                         state (`rate_a` is unlimited); export cannot represent it"
                    ));
                }
                if edited.rate_a + delta_mw <= 1e-9 {
                    return Err(format!(
                        "rating delta for branch {branch} would make the line limit non-positive"
                    ));
                }
                edited.rate_a += delta_mw;
            }
        }
    }
    Ok(())
}

/// A study state written to a target format: the serialized case text, this
/// output operation's diagnostics, and the format token and file extension.
#[derive(Clone, Debug, Serialize)]
pub struct ExportedCase {
    pub text: String,
    pub diagnostics: Vec<powerio::Diagnostic>,
    pub format: String,
    pub extension: String,
}

impl std::fmt::Debug for Study {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The cached models are large; summarize rather than dump them.
        f.debug_struct("Study")
            .field("formulation", &self.formulation)
            .field("edits", &self.log.len())
            .finish_non_exhaustive()
    }
}

/// Collapse the edit log to the cumulative delta maps the model builders consume.
/// Keys fold as written — the id and uid forms of the same element stay distinct
/// entries here and accumulate onto the same element when the model applies them.
fn fold(log: &[NetworkEdit]) -> Edits {
    let mut deltas: HashMap<ElementKey, f64> = HashMap::new();
    let mut rates: HashMap<ElementKey, f64> = HashMap::new();
    for e in log {
        match e {
            NetworkEdit::AddLoad { bus, p_mw } => {
                *deltas.entry(bus.clone()).or_insert(0.0) += *p_mw
            }
            NetworkEdit::AdjustBranchRating { branch, delta_mw } => {
                *rates.entry(branch.clone()).or_insert(0.0) += *delta_mw
            }
        }
    }
    Edits { deltas, rates }
}

/// Compute the incremental edits that move from `current` to `target`, both treated as
/// absolute edit states. Every edit kind is additive, so the state is the pair of folded
/// delta maps and the step is their difference.
fn replacement_step(current: &[NetworkEdit], target: &[NetworkEdit]) -> Vec<NetworkEdit> {
    fn diff(
        current: HashMap<ElementKey, f64>,
        target: HashMap<ElementKey, f64>,
    ) -> BTreeMap<ElementKey, f64> {
        let mut diff: BTreeMap<ElementKey, f64> = target.into_iter().collect();
        for (key, mw) in current {
            *diff.entry(key).or_default() -= mw;
        }
        diff.retain(|_, mw| *mw != 0.0);
        diff
    }
    let current = fold(current);
    let target = fold(target);
    let mut step: Vec<NetworkEdit> = diff(current.deltas, target.deltas)
        .into_iter()
        .map(|(bus, p_mw)| NetworkEdit::AddLoad { bus, p_mw })
        .collect();
    step.extend(
        diff(current.rates, target.rates)
            .into_iter()
            .map(|(branch, delta_mw)| NetworkEdit::AdjustBranchRating { branch, delta_mw }),
    );
    step
}

/// One dense element axis of a committed solution: the original ids and the row
/// uids (where carried), both in dense order.
struct ResponseAxis {
    ids: Vec<usize>,
    uids: Vec<Option<String>>,
}

/// The dense bus axis of a committed solution, read off whichever per-bus block the
/// formulation populated (`lmp`/`vm`/`va`/`w`). The preview maps edited bus keys onto
/// these dense indices, so it must use the same order the committed sensitivity
/// matrix reports.
fn response_bus_axis(resp: &SolveResponse) -> ResponseAxis {
    let scalars = resp
        .lmp
        .as_deref()
        .or(resp.vm.as_deref())
        .or(resp.va.as_deref())
        .or(resp.w.as_deref());
    if let Some(s) = scalars {
        return ResponseAxis {
            ids: s.iter().map(|b| b.bus).collect(),
            uids: s.iter().map(|b| b.uid.clone()).collect(),
        };
    }
    if let Some(inj) = resp.injections.as_deref() {
        return ResponseAxis {
            ids: inj.iter().map(|b| b.bus).collect(),
            uids: inj.iter().map(|b| b.uid.clone()).collect(),
        };
    }
    ResponseAxis {
        ids: Vec::new(),
        uids: Vec::new(),
    }
}

/// The dense branch axis of a committed solution, read off the flows block — the
/// branch-axis counterpart of [`response_bus_axis`].
fn response_branch_axis(resp: &SolveResponse) -> ResponseAxis {
    match resp.flows.as_deref() {
        Some(flows) => ResponseAxis {
            ids: flows.iter().map(|f| f.branch).collect(),
            uids: flows.iter().map(|f| f.uid.clone()).collect(),
        },
        None => ResponseAxis {
            ids: Vec::new(),
            uids: Vec::new(),
        },
    }
}

/// Map edited element keys to dense indices with aligned magnitudes (MW), dropping
/// keys that are not in this case. The uid lookup is built only when a uid key is
/// present, so a numeric-id drag pays nothing for it.
fn dense_cols(axis: &ResponseAxis, mag: &HashMap<ElementKey, f64>) -> (Vec<usize>, Vec<f64>) {
    let idx: HashMap<usize, usize> = axis
        .ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();
    let uid_idx: Option<HashMap<&str, usize>> = mag
        .keys()
        .any(|k| matches!(k, ElementKey::Uid(_)))
        .then(|| {
            axis.uids
                .iter()
                .enumerate()
                .filter_map(|(i, uid)| uid.as_deref().map(|u| (u, i)))
                .collect()
        });
    let mut dense_magnitudes = BTreeMap::<usize, f64>::new();
    for (key, &m) in mag {
        let dense = match key {
            ElementKey::Id(id) => usize::try_from(*id)
                .ok()
                .and_then(|id| idx.get(&id).copied()),
            ElementKey::Uid(uid) => uid_idx
                .as_ref()
                .and_then(|ix| ix.get(uid.as_str()).copied()),
        };
        if let Some(i) = dense {
            *dense_magnitudes.entry(i).or_default() += m;
        }
    }
    dense_magnitudes.into_iter().unzip()
}

/// Sum two per-operand prediction sets elementwise. The row axis is the operand's own
/// (identical across parameter groups), so the merge is positional; a group whose edits
/// named no known element carries empty values and defers to the other side.
fn merge_preview_columns(
    mut acc: Vec<PreviewColumn>,
    other: Vec<PreviewColumn>,
) -> Result<Vec<PreviewColumn>, String> {
    if acc.len() != other.len() {
        return Err("preview merge: operand count mismatch".into());
    }
    for (a, o) in acc.iter_mut().zip(other) {
        if a.values.is_empty() {
            *a = o;
            continue;
        }
        if o.values.is_empty() {
            continue;
        }
        if a.values.len() != o.values.len() {
            return Err("preview merge: operand row mismatch".into());
        }
        for (av, ov) in a.values.iter_mut().zip(o.values) {
            av.value += ov.value;
        }
    }
    Ok(acc)
}

/// For each watched operand, run the `parameter` sensitivity over the edited buses and dot
/// it with the edit step to get the predicted operand change (in served units).
fn preview_columns(
    sys: &dyn Differentiable,
    parameter: Parameter,
    cols: &[usize],
    col_mag: &[f64],
    watched: &[Operand],
) -> Result<Vec<PreviewColumn>, String> {
    if cols.is_empty() {
        // No (known) edited bus: every predicted change is zero.
        return Ok(watched
            .iter()
            .map(|&operand| PreviewColumn {
                operand,
                values: Vec::new(),
                units: String::new(),
            })
            .collect());
    }

    let reqs: Vec<SensRequest> = watched
        .iter()
        .map(|&operand| SensRequest {
            operand,
            parameter,
            indices: Some(cols.to_vec()),
            mode: Mode::Auto,
        })
        .collect();
    let mats = run_cells(sys, &reqs)?;

    Ok(watched
        .iter()
        .zip(mats)
        .map(|(&operand, m)| {
            // values[r][c] = d(operand_r)/d(parameter at cols[c]); the column order matches
            // col_mag, so the predicted change is the row dotted with the edit step.
            let values = m
                .values
                .iter()
                .zip(&m.rows)
                .map(|(row, meta)| PreviewValue {
                    element: meta.element,
                    index: meta.index,
                    value: row.iter().zip(col_mag).map(|(&x, &mw)| x * mw).sum(),
                })
                .collect();
            PreviewColumn {
                operand,
                values,
                units: operand_unit(&m.units),
            }
        })
        .collect())
}

/// The served unit of a predicted operand delta. The sensitivity is `(operand)/MW` or
/// `(operand)/MVA` (differentiated w.r.t. active demand or a thermal rating); the
/// predicted value is already multiplied by the MW step, so it carries the operand
/// unit — strip the denominator and parens.
fn operand_unit(ratio: &str) -> String {
    let s = ratio
        .strip_suffix("/MW")
        .or_else(|| ratio.strip_suffix("/MVA"))
        .unwrap_or(ratio)
        .trim();
    s.strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(s)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn module_json(network: BalancedNetwork) -> String {
        let module = PioModule::new(PioValue::BalancedNetwork(network));
        crate::ir::serialize_module(&module).expect("write module")
    }

    fn case3_network() -> BalancedNetwork {
        crate::model::parse_matpower(crate::model::CASE3).expect("parse")
    }

    fn case3_network_json() -> String {
        case3_network().to_json().expect("to_json")
    }

    fn case3_json() -> String {
        module_json(case3_network())
    }

    #[test]
    fn commit_matches_module_solve() {
        // A Study commit is the stateful face of the same driver: the response is
        // byte-identical to the stateless module solve at the same operating point.
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let resp = s
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 50.0,
            }])
            .expect("commit");
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}}}"#,
        )
        .expect("module solve");
        assert_eq!(from_study, stateless);
    }

    #[test]
    fn commit_with_sensitivities_matches_module_solve() {
        // A commit that carries a Price/Demand cell returns the sensitivity column in the
        // same solve, byte-equal to the same stateless module request — so the
        // frontend never needs a second solve for the ∂LMP/∂d column.
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let resp = s
            .commit_with(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 50.0,
                }],
                &[SensRequest {
                    operand: Operand::Price(Power::Active),
                    parameter: Parameter::Demand(Power::Active),
                    indices: Some(vec![1]),
                    mode: Mode::Auto,
                }],
            )
            .expect("commit_with");
        // The cell is present in the committed response (no second solve needed).
        assert_eq!(resp.sensitivities.len(), 1);
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}},"sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}]}"#,
        )
        .expect("module solve");
        assert_eq!(from_study, stateless);
    }

    #[test]
    fn feasibility_study_withholds_economic_previews() {
        let mut network = case3_network();
        for generator in network.generators_mut() {
            generator.cost = None;
        }
        let module = module_json(network);
        let study = Study::new(&module, Problem::DcOpf).expect("feasibility study");
        let edit = NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 1.0,
        };

        let price_error = study
            .preview(
                std::slice::from_ref(&edit),
                &[Operand::Price(Power::Active)],
            )
            .expect_err("a feasibility dual must not be exposed as a price");
        assert!(
            price_error.contains("network_generator_cost"),
            "{price_error}"
        );

        let preview = study.preview(&[edit], &[]).expect("physical preview");
        assert_eq!(preview.objective_delta, None);
    }

    /// CASE3 with explicit PowerIO row UIDs for keyed edit coverage.
    fn case3_with_uids_network() -> BalancedNetwork {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        for (i, b) in net.buses_mut().iter_mut().enumerate() {
            b.uid = Some(format!("buses:{i}"));
        }
        for (i, br) in net.branches_mut().iter_mut().enumerate() {
            br.uid = Some(format!("branches:{i}"));
        }
        net
    }

    fn case3_with_uids_json() -> String {
        module_json(case3_with_uids_network())
    }

    #[test]
    fn uid_keyed_edits_commit_and_preview_like_id_keyed_edits() {
        // Bus id 2 is row 1 (`buses:1`): the same drag addressed by uid must commit
        // to the same operating point and preview the same first-order column.
        let net = case3_with_uids_json();
        let mut by_id = Study::new(&net, Problem::DcOpf).expect("study");
        let mut by_uid = Study::new(&net, Problem::DcOpf).expect("study");

        let id_edit = NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 50.0,
        };
        let uid_edit = NetworkEdit::AddLoad {
            bus: ElementKey::Uid("buses:1".into()),
            p_mw: 50.0,
        };

        let watched = [Operand::Price(Power::Active)];
        let p_id = by_id
            .preview(std::slice::from_ref(&id_edit), &watched)
            .expect("preview");
        let p_uid = by_uid
            .preview(std::slice::from_ref(&uid_edit), &watched)
            .expect("preview");
        assert_eq!(
            serde_json::to_string(&p_id).unwrap(),
            serde_json::to_string(&p_uid).unwrap()
        );

        let r_id = by_id.commit(&[id_edit]).unwrap();
        let r_uid = by_uid.commit(&[uid_edit]).unwrap();
        assert_eq!(
            serde_json::to_string(&r_id).unwrap(),
            serde_json::to_string(&r_uid).unwrap()
        );
    }

    #[test]
    fn unknown_uid_key_fails_commit_and_keeps_the_committed_point() {
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let before = serde_json::to_string(s.solution()).unwrap();
        let err = s
            .commit(&[NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:99".into()),
                p_mw: 10.0,
            }])
            .unwrap_err();
        assert!(
            err.contains(r#"unknown demand delta bus "buses:99""#),
            "got: {err}"
        );
        assert_eq!(before, serde_json::to_string(s.solution()).unwrap());
        assert!(s.edits().is_empty());
    }

    #[test]
    fn duplicate_uids_reject_before_a_study_is_constructed() {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        net.buses_mut()[0].uid = Some("duplicate".into());
        net.buses_mut()[1].uid = Some("duplicate".into());
        let error = match Study::from_network(&net, Problem::DcOpf) {
            Ok(_) => panic!("duplicate base identity must reject"),
            Err(error) => error,
        };
        assert!(error.contains("duplicate bus uid"), "{error}");
    }

    #[test]
    fn an_explicit_duplicate_uid_rejects_construction() {
        // Two buses sharing a uid make edit keys ambiguous, so the session
        // refuses to construct rather than letting an ambiguous key land on
        // the wrong row later.
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        net.buses_mut()[0].uid = Some("buses:1".into());
        net.buses_mut()[1].uid = Some("buses:1".into());

        let error = Study::from_network(&net, Problem::DcOpf)
            .expect_err("a duplicate uid must reject the study");
        assert!(error.contains("duplicate bus uid"), "{error}");
    }

    #[test]
    fn study_rejects_display_only_three_winding_rows_before_they_enter_the_log() {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        let mut windings = [1, 2, 3].map(|id| powerio::Winding::new(powerio::BusId(id)));
        for winding in &mut windings {
            winding.rate_a = net.base_mva();
        }
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva());
        net.transformers_3w_mut()
            .push(powerio::Transformer3W::new(windings, [impedance; 3]));
        let synthetic_bus = net.buses().iter().map(|bus| bus.id.0).max().unwrap() + 1;
        let synthetic_branch = net.branches().len() + 1;
        let mut study = Study::from_network(&net, Problem::DcOpf).expect("3W study");
        let before = serde_json::to_string(study.solution()).unwrap();

        let bus_error = study
            .commit(&[NetworkEdit::AddLoad {
                bus: (synthetic_bus as i64).into(),
                p_mw: 1.0,
            }])
            .unwrap_err();
        assert!(
            bus_error.contains("unknown demand delta bus"),
            "{bus_error}"
        );

        let branch_error = study
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: (synthetic_branch as i64).into(),
                delta_mw: 1.0,
            }])
            .unwrap_err();
        assert!(
            branch_error.contains("unknown rating delta branch"),
            "{branch_error}"
        );
        assert!(study.edits().is_empty());
        assert_eq!(before, serde_json::to_string(study.solution()).unwrap());
    }

    #[test]
    fn preview_and_commit_reject_rows_omitted_from_analysis() {
        let mut isolated = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        isolated.buses_mut()[2].kind = powerio::BusType::Isolated;
        let isolated_id = isolated.buses()[2].id.0 as i64;
        let mut study = Study::from_network(&isolated, Problem::DcOpf).expect("isolated study");
        let bus_edit = NetworkEdit::AddLoad {
            bus: isolated_id.into(),
            p_mw: 1.0,
        };
        let preview_error = study
            .preview(
                std::slice::from_ref(&bus_edit),
                &[Operand::Price(Power::Active)],
            )
            .expect_err("isolated preview must reject");
        assert!(preview_error.contains("not editable"), "{preview_error}");
        let commit_error = study
            .commit(&[bus_edit])
            .expect_err("isolated commit must reject");
        assert!(commit_error.contains("not editable"), "{commit_error}");

        let mut inactive = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        inactive.branches_mut()[0].in_service = false;
        let mut study = Study::from_network(&inactive, Problem::DcOpf).expect("inactive study");
        let branch_edit = NetworkEdit::AdjustBranchRating {
            branch: 1.into(),
            delta_mw: 1.0,
        };
        let preview_error = study
            .preview(
                std::slice::from_ref(&branch_edit),
                &[Operand::Price(Power::Active)],
            )
            .expect_err("inactive preview must reject");
        assert!(preview_error.contains("not editable"), "{preview_error}");
        let commit_error = study
            .commit(&[branch_edit])
            .expect_err("inactive commit must reject");
        assert!(commit_error.contains("not editable"), "{commit_error}");
    }

    #[test]
    fn edits_accumulate_across_commits() {
        let net = case3_json();
        let mut a = Study::new(&net, Problem::DcOpf).unwrap();
        a.commit(&[NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 30.0,
        }])
        .unwrap();
        let two = a
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 20.0,
            }])
            .unwrap();
        assert_eq!(a.edits().len(), 2);
        // Two commits of +30 then +20 reach the same point as one +50.
        let mut b = Study::new(&net, Problem::DcOpf).unwrap();
        let once = b
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 50.0,
            }])
            .unwrap();
        assert_eq!(
            serde_json::to_string(&two).unwrap(),
            serde_json::to_string(&once).unwrap()
        );
    }

    #[test]
    fn replace_edits_sets_absolute_operating_point() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(&[NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 30.0,
        }])
        .unwrap();
        let replaced = s
            .replace_edits(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 50.0,
            }])
            .unwrap();

        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}}}"#,
        )
        .expect("module solve");
        assert_eq!(serde_json::to_string(&replaced).unwrap(), stateless);
        assert_eq!(s.edits().len(), 1);
    }

    #[test]
    fn replace_edits_can_reset_to_base() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(&[NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 30.0,
        }])
        .unwrap();
        let reset = s.replace_edits(&[]).unwrap();
        let base =
            crate::solve_module_json(&net, r#"{"formulation":"dcopf"}"#).expect("solve module");
        assert_eq!(serde_json::to_string(&reset).unwrap(), base);
        assert!(s.edits().is_empty());
    }

    #[test]
    fn failed_replace_keeps_last_committed_point() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.replace_edits(&[NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 30.0,
        }])
        .unwrap();
        let committed = serde_json::to_string(s.solution()).unwrap();

        let err = s
            .replace_edits(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 1_000_000.0,
            }])
            .unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(s.edits().len(), 1);
        assert_eq!(serde_json::to_string(s.solution()).unwrap(), committed);
    }

    #[test]
    fn preview_replacement_uses_delta_from_committed_point() {
        let net = case3_json();
        let mut absolute = Study::new(&net, Problem::DcOpf).unwrap();
        absolute
            .replace_edits(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }])
            .unwrap();
        let toward_fifty = absolute
            .preview_replacement(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 50.0,
                }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();

        let mut incremental = Study::new(&net, Problem::DcOpf).unwrap();
        incremental
            .replace_edits(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }])
            .unwrap();
        let plus_twenty = incremental
            .preview(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 20.0,
                }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();

        assert_eq!(
            serde_json::to_string(&toward_fifty).unwrap(),
            serde_json::to_string(&plus_twenty).unwrap()
        );
    }

    #[test]
    fn preview_is_first_order_accurate_for_a_small_step() {
        // The preview at the committed (base) point predicts the LMP change of a small
        // demand step; the DC OPF QP is smooth, so first order ≈ the exact commit.
        let net = case3_json();
        let study = Study::new(&net, Problem::DcOpf).unwrap();
        let step = 1.0_f64; // MW
        let prev = study
            .preview(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: step,
                }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();
        assert!(prev.local_only);
        assert_eq!(prev.operands.len(), 1);
        assert_eq!(prev.operands[0].units, "objective_unit/MW");

        let base: Value =
            serde_json::from_str(&serde_json::to_string(study.solution()).unwrap()).unwrap();
        let mut committed_study = Study::new(&net, Problem::DcOpf).unwrap();
        let committed = committed_study
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: step,
            }])
            .unwrap();
        let committed_json: Value =
            serde_json::from_str(&serde_json::to_string(&committed).unwrap()).unwrap();

        // Compare predicted ΔLMP to the exact ΔLMP bus by bus.
        for col in &prev.operands[0].values {
            let bus = match col.element {
                ElementId::Bus(b) => b,
                _ => panic!("price operand should be bus-keyed"),
            };
            let base_lmp = lmp_at(&base, bus);
            let new_lmp = lmp_at(&committed_json, bus);
            let exact = new_lmp - base_lmp;
            assert!(
                (col.value - exact).abs() < 1e-3,
                "bus {bus}: predicted Δlmp {} vs exact {exact}",
                col.value
            );
        }
        // Adding load raises system cost: the objective gradient is positive.
        assert!(prev.objective_delta.unwrap() > 0.0);
    }

    #[test]
    fn commit_with_rating_edit_matches_module_solve() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let resp = s
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -210.0,
            }])
            .expect("commit");
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"rates":{"3":-210.0}}}"#,
        )
        .expect("module solve");
        assert_eq!(serde_json::to_string(&resp).unwrap(), stateless);
    }

    #[test]
    fn rating_preview_is_first_order_accurate_on_a_binding_line() {
        // Congest the bus2-bus3 line through the rating edit itself (250 -> 40 MW,
        // the same operating point as sens/dc.rs's congested_case3), then preview a
        // further 1 MW tightening against the exact re-solve.
        let net = case3_json();
        let mut study = Study::new(&net, Problem::DcOpf).unwrap();
        study
            .replace_edits(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -210.0,
            }])
            .unwrap();

        let step = -1.0_f64; // MW
        let prev = study
            .preview(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: step,
                }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();
        assert_eq!(prev.operands.len(), 1);
        assert_eq!(prev.operands[0].units, "objective_unit/MW");

        let committed: Value =
            serde_json::from_str(&serde_json::to_string(study.solution()).unwrap()).unwrap();
        let mut exact_study = Study::new(&net, Problem::DcOpf).unwrap();
        let exact = exact_study
            .replace_edits(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -210.0 + step,
            }])
            .unwrap();
        let exact_json: Value =
            serde_json::from_str(&serde_json::to_string(&exact).unwrap()).unwrap();

        let mut moved = 0.0_f64;
        for col in &prev.operands[0].values {
            let bus = match col.element {
                ElementId::Bus(b) => b,
                _ => panic!("price operand should be bus-keyed"),
            };
            let exact_delta = lmp_at(&exact_json, bus) - lmp_at(&committed, bus);
            assert!(
                (col.value - exact_delta).abs() < 1e-3,
                "bus {bus}: predicted Δlmp {} vs exact {exact_delta}",
                col.value
            );
            moved = moved.max(col.value.abs());
        }
        // The binding line's rating must actually move prices, so this is a real
        // validation rather than a trivial 0 == 0.
        assert!(
            moved > 1e-5,
            "rating preview at binding line is trivial: {moved}"
        );

        // Tightening a binding limit raises cost, and the gradient objective agrees
        // with the exact re-solve to first order.
        let pred_obj = prev.objective_delta.expect("dc opf has an objective");
        let exact_obj =
            exact_json["objective"].as_f64().unwrap() - committed["objective"].as_f64().unwrap();
        assert!(
            pred_obj > 0.0,
            "tightening a binding limit should raise cost"
        );
        assert!(
            (pred_obj - exact_obj).abs() <= 0.15 * exact_obj.abs() + 1e-9,
            "objective gradient {pred_obj} vs exact {exact_obj}"
        );
    }

    #[test]
    fn rating_preview_on_a_non_binding_line_is_zero() {
        // Uncongested base: no line binds, so the rating column and its objective
        // gradient are exactly zero.
        let net = case3_json();
        let study = Study::new(&net, Problem::DcOpf).unwrap();
        let prev = study
            .preview(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -10.0,
                }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();
        for v in &prev.operands[0].values {
            assert!(v.value.abs() < 1e-9, "expected zero, got {}", v.value);
        }
        assert!(prev.objective_delta.unwrap().abs() < 1e-9);
    }

    #[test]
    fn mixed_demand_and_rating_edits_fold_reset_and_preview() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        let resp = s
            .commit(&[
                NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 20.0,
                },
                NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -210.0,
                },
            ])
            .unwrap();
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":20.0},"rates":{"3":-210.0}}}"#,
        )
        .unwrap();
        assert_eq!(serde_json::to_string(&resp).unwrap(), stateless);

        // A mixed preview is the sum of the groups' first-order terms (this used to
        // error on mixed edit kinds, which replacement_step now legitimately emits).
        let prev = s
            .preview(
                &[
                    NetworkEdit::AddLoad {
                        bus: 2.into(),
                        p_mw: 1.0,
                    },
                    NetworkEdit::AdjustBranchRating {
                        branch: 3.into(),
                        delta_mw: -1.0,
                    },
                ],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();
        assert_eq!(prev.operands.len(), 1);
        assert!(!prev.operands[0].values.is_empty());
        assert!(prev.objective_delta.is_some());

        // Resetting a mixed log reconstructs the base solve (replacement_step emits
        // both edit kinds).
        let reset = s.replace_edits(&[]).unwrap();
        let base = crate::solve_module_json(&net, r#"{"formulation":"dcopf"}"#).unwrap();
        assert_eq!(serde_json::to_string(&reset).unwrap(), base);
        assert!(s.edits().is_empty());
    }

    #[test]
    fn rating_edit_validation_errors() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        let err = s
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 99.into(),
                delta_mw: -10.0,
            }])
            .unwrap_err();
        assert!(err.contains("unknown rating delta branch 99"), "got: {err}");
        let err = s
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -250.0,
            }])
            .unwrap_err();
        assert!(
            err.contains("would make the line limit non-positive"),
            "got: {err}"
        );
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn acpf_rejects_rating_edits() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::AcPf).expect("acpf study");
        let err = s
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -10.0,
            }])
            .unwrap_err();
        assert!(
            err.contains("branch rating edits are not supported by acpf"),
            "got: {err}"
        );
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_commit_with_rating_edit_matches_module_solve() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::Socwr).expect("socwr study");
        let resp = s
            .commit(&[NetworkEdit::AdjustBranchRating {
                branch: 3.into(),
                delta_mw: -210.0,
            }])
            .expect("commit");
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"socwr","edits":{"rates":{"3":-210.0}}}"#,
        )
        .expect("module solve");
        assert_eq!(serde_json::to_string(&resp).unwrap(), stateless);
    }

    #[test]
    fn preview_without_an_edit_is_zero() {
        let net = case3_json();
        let study = Study::new(&net, Problem::DcOpf).unwrap();
        let prev = study
            .preview(&[], &[Operand::Price(Power::Active)])
            .unwrap();
        assert_eq!(prev.objective_delta, Some(0.0));
        assert!(prev.operands[0].values.is_empty());
    }

    #[test]
    fn preview_works_for_ac_pf_study() {
        // An AC power flow study has no objective, so the preview's objective_delta is
        // None, but the watched voltage operand still gets a finite first-order column.
        let net = case3_json();
        let study = Study::new(&net, Problem::AcPf).expect("acpf study");
        let prev = study
            .preview(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 1.0,
                }],
                &[Operand::Voltage(crate::sens::VoltageKind::Magnitude)],
            )
            .expect("acpf preview");
        assert!(prev.local_only);
        assert!(
            prev.objective_delta.is_none(),
            "power flow has no objective"
        );
        assert_eq!(prev.operands.len(), 1);
        assert_eq!(prev.operands[0].units, "pu");
        for v in &prev.operands[0].values {
            assert!(v.value.is_finite());
        }
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_study_constructs_and_commits() {
        // A SOCWR study is constructible and commits successfully through the boxed state
        // (the conic KKT builds on the stack in with_system / socwr_assemble), and the
        // commit is byte-equal to the same stateless module request.
        let net = case3_json();
        let mut s = Study::new(&net, Problem::Socwr).expect("socwr study");
        assert_eq!(s.formulation(), Problem::Socwr);
        let resp = s
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 10.0,
            }])
            .expect("socwr commit");
        assert!(resp.w.is_some(), "socwr reports w");
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_module_json(
            &net,
            r#"{"formulation":"socwr","edits":{"deltas":{"2":10.0}}}"#,
        )
        .expect("module solve");
        assert_eq!(from_study, stateless);
    }

    #[cfg(not(feature = "conic"))]
    #[test]
    fn study_rejects_unbuilt_formulation() {
        // Without the conic feature SOCWR is not in this build.
        let err = Study::new(&case3_json(), Problem::Socwr).unwrap_err();
        assert!(err.contains("cannot start a Socwr Study"), "got: {err}");
    }

    fn lmp_at(v: &Value, bus: usize) -> f64 {
        v["lmp"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["bus"].as_u64() == Some(bus as u64))
            .map(|e| e["value"].as_f64().unwrap())
            .unwrap_or_else(|| panic!("no lmp for bus {bus}"))
    }

    // -----------------------------------------------------------------------
    // Module save / export
    // -----------------------------------------------------------------------

    #[test]
    fn study_rejects_bare_model_json() {
        let error = Study::new(&case3_network_json(), Problem::DcOpf)
            .expect_err("a Study requires a retained PowerIO module");
        assert!(!error.is_empty());
    }

    fn dcopf_objective(network_json: &str) -> f64 {
        let network = BalancedNetwork::from_json(network_json).expect("network JSON");
        let module = module_json(network);
        let out = crate::solve_module_json(&module, r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        v["objective"].as_f64().unwrap()
    }

    #[test]
    fn save_module_writes_the_materialized_network_with_edit_history() {
        // A demand edit and a rating edit save as one stored PowerIO module:
        // the value is the edited network, the retained producer stays intact, and each
        // committed edit is one descriptive Edit history entry. No tellegen
        // format wraps the module.
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(&[
            NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 20.0,
            },
            NetworkEdit::AdjustBranchRating {
                branch: ElementKey::Uid("branches:2".into()),
                delta_mw: -210.0,
            },
        ])
        .unwrap();

        let text = s.save_module().unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema"], "powerio.module");
        assert_eq!(value["value"]["type"], "powerio.BalancedNetwork");
        assert_eq!(value["producer"]["name"], "powerio");
        let history = value["history"].as_array().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0]["kind"], "edit");
        assert_eq!(history[0]["name"], "tellegen.add_load");
        assert_eq!(history[0]["parameters"]["bus"], "buses:1");
        assert_eq!(history[0]["parameters"]["p_mw"], 20.0);
        assert_eq!(history[1]["name"], "tellegen.adjust_branch_rating");
        assert_eq!(history[1]["parameters"]["delta_mw"], -210.0);
    }

    #[test]
    fn save_solution_module_embeds_the_amended_instance_that_was_solved() {
        let mut study = Study::new(&case3_json(), Problem::DcOpf).unwrap();
        let committed = study
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 25.0,
            }])
            .unwrap();

        let module = crate::ir::deserialize_module(&study.save_solution_module().unwrap()).unwrap();
        let PioValue::DcOpfSolution(solution) = module.into_value() else {
            panic!("saved value is not a DC OPF solution");
        };
        let amended_load: f64 = solution
            .instance()
            .network()
            .loads()
            .iter()
            .filter(|load| load.in_service && load.bus.0 == 2)
            .map(|load| load.p)
            .sum();
        let base_load: f64 = case3_network()
            .loads()
            .iter()
            .filter(|load| load.in_service && load.bus.0 == 2)
            .map(|load| load.p)
            .sum();
        assert!((amended_load - base_load - 25.0).abs() < 1e-9);
        assert_eq!(solution.objective(), committed.objective.unwrap());
    }

    #[test]
    fn save_module_retains_existing_module_records_and_uses_unique_history_ids() {
        use powerio::{
            Diagnostic, DiagnosticCode, DiagnosticSeverity, HistoryEntry, HistoryId, HistoryKind,
        };

        let network = case3_with_uids_network();
        let producer = powerio::Producer::new("source-tool", "1.2.3").unwrap();
        let mut module = PioModule::new(network).with_producer(producer);
        module
            .insert_extension("org.example.keep", serde_json::json!({"answer": 42}))
            .unwrap();
        module
            .add_history_entry(
                HistoryEntry::new(
                    HistoryId::new("tellegen-edit-1").unwrap(),
                    HistoryKind::Parse,
                    "source-tool.parse",
                )
                .unwrap(),
            )
            .unwrap();
        module
            .add_diagnostic(
                Diagnostic::new(
                    DiagnosticCode::new("READ.TEST.NOTE").unwrap(),
                    DiagnosticSeverity::Warning,
                    "retained finding",
                )
                .with_target("/loads/0/p")
                .unwrap(),
            )
            .unwrap();

        // Exercise obsolete source linkage through the public stored module
        // boundary. SourceMapEntry is intentionally not required from a
        // component crate merely to consume a module through the facade.
        let dynamic = module.map_value(PioValue::BalancedNetwork);
        let mut stored: Value = serde_json::from_str(
            &crate::ir::serialize_module(&dynamic).expect("write source module"),
        )
        .expect("stored JSON");
        stored["source_map"] = serde_json::json!([{
            "target": "/loads/0/p",
            "relation": "synthetic"
        }]);
        let module =
            crate::ir::deserialize_module(&serde_json::to_string(&stored).expect("stored JSON"))
                .expect("read source module");
        let module = crate::ir::balanced_module(module).expect("typed source module");

        let mut study = Study::from_module(module, Problem::DcOpf).unwrap();
        let exported = study.export("matpower").unwrap();
        assert!(
            exported
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code() != "READ.TEST.NOTE"),
            "parse diagnostics stay on the module instead of becoming output diagnostics"
        );
        study
            .commit(&[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 5.0,
            }])
            .unwrap();

        let saved = crate::ir::deserialize_module(&study.save_module().unwrap()).unwrap();
        assert_eq!(saved.producer().name(), "source-tool");
        assert_eq!(saved.producer().version(), "1.2.3");
        assert_eq!(saved.extensions()["org.example.keep"]["answer"], 42);
        assert_eq!(saved.history().len(), 2);
        assert_eq!(saved.history()[0].id().as_str(), "tellegen-edit-1");
        assert_eq!(saved.history()[1].id().as_str(), "tellegen-edit-2");
        assert!(saved.sources().is_empty(), "{:?}", saved.sources());
        assert!(saved.source_map().is_empty());
        assert_eq!(saved.diagnostics.len(), 1);
        assert_eq!(saved.diagnostics[0].message(), "retained finding");
        assert!(saved.diagnostics[0].target().is_none());
    }

    #[test]
    fn a_saved_module_reads_back_as_a_fresh_base_at_the_committed_point() {
        // The saved value is materialized: reading the module back through
        // powerio and starting a new session solves to the committed
        // objective with an empty edit log — history is descriptive, never
        // interpreted as state.

        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(&[NetworkEdit::AddLoad {
            bus: 2.into(),
            p_mw: 50.0,
        }])
        .unwrap();
        let committed = s.solution().objective.unwrap();

        let module = crate::ir::deserialize_module(&s.save_module().unwrap()).unwrap();
        let module: PioModule<BalancedNetwork> = crate::ir::balanced_module(module).unwrap();
        let restored = Study::from_module(module, Problem::DcOpf).unwrap();
        assert!(restored.edits().is_empty());
        assert_eq!(restored.commits(), 0);
        assert!((restored.solution().objective.unwrap() - committed).abs() < 1e-9);
    }

    #[test]
    fn save_module_with_no_edits_writes_the_base_and_no_history() {
        let s = Study::new(&case3_json(), Problem::DcOpf).unwrap();
        let value: Value = serde_json::from_str(&s.save_module().unwrap()).unwrap();
        assert_eq!(value["value"]["type"], "powerio.BalancedNetwork");
        assert!(value["history"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty());
        assert!(
            value["value"].is_object(),
            "module value payload: {}",
            value["value"]
        );
    }

    #[test]
    fn export_of_the_committed_point_reparses_and_solves_to_the_same_objective() {
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(&[NetworkEdit::AddLoad {
            bus: ElementKey::Uid("buses:1".into()),
            p_mw: 50.0,
        }])
        .unwrap();
        let committed = s.solution().objective.unwrap();

        // The exact snapshot format re-solves to the identical objective.
        let pio = s.export("model-json").unwrap();
        assert!((dcopf_objective(&pio.text) - committed).abs() < 1e-9);

        // MATPOWER reparses to the same materialized model (to writer precision).
        let m = s.export("matpower").unwrap();
        assert_eq!(m.extension, "m");
        let reparsed = crate::model::parse_matpower(&m.text)
            .unwrap()
            .to_json()
            .unwrap();
        assert!((dcopf_objective(&reparsed) - committed).abs() < 1e-3);
    }

    #[test]
    fn export_with_no_commits_writes_the_base_case() {
        let s = Study::new(&case3_json(), Problem::DcOpf).unwrap();
        let base = s.export("model-json").unwrap();
        assert!((dcopf_objective(&base.text) - s.solution().objective.unwrap()).abs() < 1e-9);
    }

    /// A dropped case names its own buses, and tellegen writes those names into
    /// the export a user downloads. A name holding a line terminator used to end
    /// the record early, so the rest of it parsed as further records. powerio
    /// replaces the terminator in the writer; every text target tellegen can
    /// reach must hold.
    #[test]
    fn an_export_cannot_gain_records_from_a_bus_name() {
        let mut net: Value = serde_json::from_str(&case3_network_json()).unwrap();
        net["buses"].as_array_mut().unwrap()[1]["name"] =
            serde_json::json!("A\n 999,'B',1,1,1,1,1,1.0,0.0,1.0,1.0,1.1,0.9");
        let network = BalancedNetwork::from_json(&net.to_string()).unwrap();
        let s = Study::new(&module_json(network), Problem::DcOpf).unwrap();

        for format in ["matpower", "psse", "pslf", "powerworld"] {
            let exported = s.export(format).expect(format);
            assert!(
                !exported.text.contains("\n 999,"),
                "{format}: a bus name ended its record"
            );
            assert!(
                !exported.text.contains("\r 999,"),
                "{format}: a bus name ended its record"
            );
        }
    }

    #[test]
    fn export_rejects_unknown_format() {
        let s = Study::new(&case3_json(), Problem::DcOpf).unwrap();
        let err = s.export("nonesuch").unwrap_err();
        assert!(err.contains("REQUEST.EMIT.UNKNOWN_FORMAT"), "got: {err}");
    }

    #[cfg(feature = "conic")]
    #[test]
    fn a_socwr_study_saves_and_exports_its_materialized_state() {
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::Socwr).unwrap();
        s.commit(&[NetworkEdit::AddLoad {
            bus: ElementKey::Uid("buses:1".into()),
            p_mw: 10.0,
        }])
        .unwrap();
        let value: Value = serde_json::from_str(&s.save_module().unwrap()).unwrap();
        assert_eq!(value["value"]["type"], "powerio.BalancedNetwork");
        assert_eq!(value["history"].as_array().unwrap().len(), 1);
        let exported = s.export("model-json").unwrap();
        assert!(!exported.text.is_empty());
    }
}
