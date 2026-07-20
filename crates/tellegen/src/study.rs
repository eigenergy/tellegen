//! A build-once, solve-many interactive handle over the engine — the stateful face of
//! the same driver [`solve_json`](crate::solve_json) exposes statelessly.
//!
//! Parse and build the model **once**, then [`commit`](Study::commit) exact-re-solves
//! at the new operating point and [`preview`](Study::preview) returns a first-order
//! linearization at the committed point with no re-solve. This is the reactive hot path
//! the browser pays a full network re-parse for today (`solve_json` rebuilds the network
//! on every call); the first-order column the sensitivity driver already produces *is*
//! the preview.
//!
//! A `Study` is the base case plus an ordered edit log — the unit a UI saves and replays —
//! and it fully reconstructs the current operating point by replaying that log.
//!
//! The formulation coupling is a single boxed [`SolvedState`] trait object: the same
//! [`Differentiable`] dispatch the stateless driver uses, captured once at construction
//! and at every commit. Every formulation the build includes is supported — DC OPF and
//! AC power flow always, and the SOCWR relaxation behind `conic` — and a formulation this
//! build omits returns a clean error naming `solve_json` as the stateless route.

use std::collections::{BTreeMap, HashMap};

use powerio::network::Network;
use powerio_pkg::{ElementRef, NetworkPackage, StudyBlock, StudyCommit, StudyEdit};
use serde::{Deserialize, Serialize};

use crate::api::{
    ac_pf_assemble, ac_pf_solved, dc_opf_assemble, dc_opf_solved, run_cells, Edits, ElementKey,
    Problem, SensRequest, SolveOptions, SolveRequest, SolveResponse,
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

/// Absolute demand deltas keyed by bus id and rating deltas keyed by 1-based branch
/// position, the folded edit state a UI restores after loading a saved study.
pub type FoldedDeltas = (BTreeMap<i64, f64>, BTreeMap<i64, f64>);

/// A first-order preview of an edit at the committed operating point: the predicted
/// change in each watched operand, the predicted objective change, and the
/// linearization caveat.
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    /// One predicted operand-delta column per watched operand, in request order.
    pub operands: Vec<PreviewColumn>,
    /// First-order objective change ($) along the edit, when the formulation has an
    /// objective (OPF). `None` for power flow. For a demand edit this is the committed
    /// marginal price dotted with the demand step (`Σ lmp_b · Δp_b`).
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
    /// Served-unit label of the prediction (e.g. `$/MWh`, `pu`), from the sensitivity.
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

    /// The committed marginal price in served units ($/MWh), per dense bus, for the
    /// preview's objective delta. `None` for power flow formulations (no objective).
    fn lmp(&self) -> Option<Vec<f64>>;

    /// The committed flow-limit shadow price in served units ($/MWh per MW of rating),
    /// per dense branch, for the rating preview's objective delta: ∂objective/∂rating
    /// = −shadow. `None` when the formulation exposes no flow-limit duals; the preview
    /// then reports no objective delta rather than a partial one.
    fn line_shadow_prices(&self) -> Option<Vec<f64>> {
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
        Some(self.sol.lmp_usd_per_mwh(self.net.base_mva))
    }
    fn line_shadow_prices(&self) -> Option<Vec<f64>> {
        // Both limit rows relax when the rating grows, so the shadow is the dual sum;
        // the raw duals are per-unit, and the served value divides by base (the same
        // scaling as the LMP).
        let base = self.net.base_mva;
        Some(
            self.sol
                .lam_ub
                .iter()
                .zip(&self.sol.lam_lb)
                .map(|(&ub, &lb)| (ub + lb) / base)
                .collect(),
        )
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
        // The raw conic price is the balance dual; the served LMP divides by base (the
        // same `1/base` scale the api applies in `socwr_assemble`).
        let base = self.net.base_mva;
        Some(self.sol.lmp.iter().map(|v| v / base).collect())
    }
}

/// Solve `req`'s formulation at `base + edits` from an owned [`Network`] and box the
/// committed state. Dispatches **once**, mirroring [`solve_network`](crate::solve_network):
/// the boxed [`SolvedState`] is the only formulation `match` the study performs, and a
/// formulation this build omits returns a clean `Err` naming `solve_json`.
fn solve_state(net: &Network, req: &SolveRequest) -> Result<Box<dyn SolvedState>, String> {
    match req.formulation {
        Problem::DcOpf => {
            let (net, sol) = dc_opf_solved(DcNetwork::from_network(net)?, req, None)?;
            Ok(Box::new(DcState { net, sol }))
        }
        Problem::AcPf => {
            let (net, sol) = ac_pf_solved(AcNetwork::from_network(net)?, req)?;
            Ok(Box::new(AcPfState { net, sol }))
        }
        #[cfg(feature = "conic")]
        Problem::Socwr => {
            let (net, sol) = socwr_solved(AcNetwork::from_network(net)?, req)?;
            Ok(Box::new(ConicState { net, sol }))
        }
        other => Err(format!(
            "Study does not support {other:?} in this build; use solve_json for stateless {other:?} solves"
        )),
    }
}

/// A stateful, build-once handle. Construct with a network and a formulation; the base
/// network is retained as the source of truth, and the base case is solved immediately,
/// so [`solution`](Study::solution) and [`preview`](Study::preview) are available right
/// away. Every commit re-solves from a fresh clone of that base, so the operating point
/// is always `base + the whole edit log` — never an accumulated drift.
pub struct Study {
    formulation: Problem,
    /// The parsed base network: the source of truth re-solved (cloned) at every commit.
    base: Network,
    options: SolveOptions,
    log: Vec<NetworkEdit>,
    /// Commit boundaries partitioning `log` into the batches each [`commit`](Study::commit)
    /// call appended. Each entry is a running prefix length into `log`, one per commit,
    /// non-decreasing, with `commit_bounds.last() == log.len()`. A study with no edits
    /// has no commits. The package study block maps one entry to one `StudyCommit`, so
    /// `to_package` never flattens the history into a single commit.
    commit_bounds: Vec<usize>,
    /// The committed solved state, the sole formulation coupling.
    solved: Box<dyn SolvedState>,
    last: SolveResponse,
}

impl Study {
    /// Parse `network_json` (powerio `Network` JSON), build the model for `formulation`,
    /// and solve the base case. The parse/normalize/index cost is paid once here, not on
    /// every solve.
    pub fn new(network_json: &str, formulation: Problem) -> Result<Self, String> {
        let net = Network::from_json(network_json).map_err(|e| e.to_string())?;
        Self::from_network(&net, formulation)
    }

    /// As [`new`](Study::new) from an already-parsed [`Network`].
    pub fn from_network(net: &Network, formulation: Problem) -> Result<Self, String> {
        let options = SolveOptions::default();
        let req = SolveRequest {
            formulation,
            edits: Edits::default(),
            options: options.clone(),
            ..Default::default()
        };
        let solved = solve_state(net, &req)?;
        let last = solved.assemble(&req)?;
        Ok(Study {
            formulation,
            base: net.clone(),
            options,
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
    pub fn commit(
        &mut self,
        edits: &[NetworkEdit],
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        self.commit_with(edits, &[], options)
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
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        let mut next_log = self.log.clone();
        next_log.extend_from_slice(edits);
        // Append records one commit per call, so the package study block keeps this
        // batch as its own `StudyCommit` instead of folding it into the history.
        let mut next_bounds = self.commit_bounds.clone();
        next_bounds.push(next_log.len());
        self.commit_log(next_log, next_bounds, sensitivities, options)
    }

    /// Replace the committed edit set with `edits` and exact-re-solve, with no
    /// sensitivity cells. Use this when the caller owns an absolute state such as
    /// `base demand + deltas`, rather than an append-only edit log.
    pub fn replace_edits(
        &mut self,
        edits: &[NetworkEdit],
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        self.replace_edits_with(edits, &[], options)
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
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        // Replacing the operating point discards the append history: the whole absolute
        // state becomes one commit (or none, when reset to base).
        let bounds = if edits.is_empty() {
            Vec::new()
        } else {
            vec![edits.len()]
        };
        self.commit_log(edits.to_vec(), bounds, sensitivities, options)
    }

    fn commit_log(
        &mut self,
        log: Vec<NetworkEdit>,
        commit_bounds: Vec<usize>,
        sensitivities: &[SensRequest],
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        let (solved, resp) = self.solve_log(&log, sensitivities, &options)?;
        self.options = options;
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
        options: &SolveOptions,
    ) -> Result<(Box<dyn SolvedState>, SolveResponse), String> {
        let req = SolveRequest {
            formulation: self.formulation,
            edits: fold(log),
            sensitivities: sensitivities.to_vec(),
            options: options.clone(),
        };
        // Re-solve from a fresh clone of the base (the source of truth), then assemble the
        // response — including the requested sensitivity cells — from the committed state.
        let solved = solve_state(&self.base, &req)?;
        let resp = solved.assemble(&req)?;
        Ok((solved, resp))
    }

    /// First-order prediction of applying `edits` at the committed point, for each
    /// `watched` operand, without re-solving. Builds the committed state's differentiable
    /// system **fresh** (never a cached factorization) and dots its `dz/dp` column with
    /// the edit step. The result is a local linearization (`local_only = true`); `commit`
    /// to confirm. Linearizes at the *last committed* state, not the base.
    pub fn preview(&self, edits: &[NetworkEdit], watched: &[Operand]) -> Result<Preview, String> {
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

        // First-order objective change, summed across groups: Σ lmp_b · Δp_b for a demand
        // step, −Σ μ_e · Δfmax_e for a rating step (relaxing a binding limit lowers cost).
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
        let step = replacement_step(&self.log, target);
        self.preview(&step, watched)
    }

    /// The number of committed edit batches (package study commits). A study with no
    /// edits has zero; `to_package` writes one `StudyCommit` per batch, so
    /// `commit`/`materialize` index the same `k` on both sides.
    pub fn commits(&self) -> usize {
        self.commit_bounds.len()
    }

    /// The committed solve options, for a caller persisting or restoring the study.
    pub fn solve_options(&self) -> &SolveOptions {
        &self.options
    }

    /// Fold the committed log to absolute demand deltas keyed by bus id and rating
    /// deltas keyed by 1-based branch position — the numeric ids a UI restores its
    /// sliders from after [`from_package`](Study::from_package). A uid key resolves to
    /// its element's numeric id against the base network; an unresolved key errors.
    pub fn folded_deltas_by_id(&self) -> Result<FoldedDeltas, String> {
        let mut deltas: BTreeMap<i64, f64> = BTreeMap::new();
        let mut rates: BTreeMap<i64, f64> = BTreeMap::new();
        for edit in &self.log {
            match edit {
                NetworkEdit::AddLoad { bus, p_mw } => {
                    *deltas.entry(bus_id_for_key(&self.base, bus)?).or_default() += *p_mw;
                }
                NetworkEdit::AdjustBranchRating { branch, delta_mw } => {
                    *rates
                        .entry(branch_id_for_key(&self.base, branch)?)
                        .or_default() += *delta_mw;
                }
            }
        }
        Ok((deltas, rates))
    }

    /// Serialize this study as an ordinary powerio package: the base network is the
    /// payload (row uids stamped via `ensure_payload_uids`), the edit log is the study
    /// block (one `StudyCommit` per commit call, each edit keyed by the row's actual
    /// uid), and the formulation plus solve options ride under `study.app["tellegen"]`
    /// as a versioned blob. [`from_package`](Study::from_package) is the inverse.
    pub fn to_package(&self) -> Result<NetworkPackage, String> {
        // Stamp uids on a clone of the base so edit keys resolve to the exact uids the
        // payload will carry; source-format uids (e.g. GOC3) are left untouched.
        let mut net = self.base.clone();
        powerio_pkg::ensure_payload_uids(&mut net);

        let mut commits: Vec<StudyCommit> = Vec::with_capacity(self.commit_bounds.len());
        let mut start = 0usize;
        for &end in &self.commit_bounds {
            let edits = self.log[start..end]
                .iter()
                .map(|e| study_edit_from_network_edit(&net, e))
                .collect::<Result<Vec<_>, String>>()?;
            let mut commit = StudyCommit::default();
            commit.edits = edits;
            commits.push(commit);
            start = end;
        }

        let app = TellegenApp {
            schema_version: TELLEGEN_APP_SCHEMA_VERSION,
            formulation: self.formulation,
            options: self.options.clone(),
        };
        let app_value = serde_json::to_value(&app).map_err(|e| e.to_string())?;

        let mut study = StudyBlock::default();
        study.commits = commits;
        study.app.insert(TELLEGEN_APP_KEY.to_owned(), app_value);

        let mut package = NetworkPackage::from_balanced(net);
        package.set_study(study);
        Ok(package)
    }

    /// Reconstruct a study from a package [`to_package`](Study::to_package) wrote: the
    /// balanced payload becomes the base case, `study.app["tellegen"]` restores the
    /// formulation and solve options, and each `StudyCommit`'s edits replay as one
    /// commit batch. Fails closed with a typed error — never a silent drop — on a
    /// non-balanced payload, a missing or wrong-version `app["tellegen"]` blob, an edit
    /// kind tellegen does not model, or an edit key that does not resolve.
    pub fn from_package(package: &NetworkPackage) -> Result<Study, String> {
        let net = package.as_balanced().ok_or(
            "package payload is not balanced; a tellegen study requires a balanced network",
        )?;

        let study = package
            .study()
            .ok_or("package has no study block; not a tellegen study package")?;
        let app_value = study.app.get(TELLEGEN_APP_KEY).ok_or(
            "package study has no app[\"tellegen\"] metadata; not a tellegen study package",
        )?;
        let app: TellegenApp = serde_json::from_value(app_value.clone())
            .map_err(|e| format!("unrecognized app[\"tellegen\"] payload: {e}"))?;
        if app.schema_version != TELLEGEN_APP_SCHEMA_VERSION {
            return Err(format!(
                "unsupported app[\"tellegen\"] schema_version {}; this build reads version {}",
                app.schema_version, TELLEGEN_APP_SCHEMA_VERSION
            ));
        }

        let mut log: Vec<NetworkEdit> = Vec::new();
        let mut commit_bounds: Vec<usize> = Vec::with_capacity(study.commits.len());
        for (commit_pos, commit) in study.commits.iter().enumerate() {
            for (edit_pos, edit) in commit.edits.iter().enumerate() {
                log.push(
                    network_edit_from_study_edit(net, edit)
                        .map_err(|e| format!("study commit {commit_pos} edit {edit_pos}: {e}"))?,
                );
            }
            commit_bounds.push(log.len());
        }

        // Solve the base, then replay the whole log under the restored options. A key
        // that fails to resolve (or a case that will not solve) surfaces here as an
        // `Err`, so a bad package never yields a half-built study.
        let mut restored = Study::from_network(net, app.formulation)?;
        restored.commit_log(log, commit_bounds, &[], app.options)?;
        Ok(restored)
    }
}

/// The `app["tellegen"]` blob: the formulation and solve options a package carries so
/// [`Study::from_package`] restores the same study, versioned so an older reader
/// rejects a payload it does not understand rather than guessing.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TellegenApp {
    schema_version: u32,
    formulation: Problem,
    options: SolveOptions,
}

const TELLEGEN_APP_KEY: &str = "tellegen";
const TELLEGEN_APP_SCHEMA_VERSION: u32 = 1;

/// Map one tellegen [`NetworkEdit`] to a powerio [`StudyEdit`], keyed by the resolved
/// row's actual uid. Errors if the edit names an element the base network does not
/// carry — a study must not persist a dangling edit.
fn study_edit_from_network_edit(net: &Network, edit: &NetworkEdit) -> Result<StudyEdit, String> {
    match edit {
        NetworkEdit::AddLoad { bus, p_mw } => {
            let uid = bus_uid_for_key(net, bus)?;
            Ok(StudyEdit::DemandDelta {
                bus: ElementRef::by_source_uid("buses", uid),
                p_mw: *p_mw,
                q_mvar: None,
            })
        }
        NetworkEdit::AdjustBranchRating { branch, delta_mw } => {
            let uid = branch_uid_for_key(net, branch)?;
            Ok(StudyEdit::RatingDelta {
                branch: ElementRef::by_source_uid("branches", uid),
                delta_mw: *delta_mw,
            })
        }
    }
}

/// Map one powerio [`StudyEdit`] back to a tellegen [`NetworkEdit`]. Every rejection is
/// a typed error so [`Study::from_package`] fails closed: an unknown or unsupported
/// edit kind, a reactive demand delta tellegen cannot model, or a reference that names
/// no element all reject the package.
fn network_edit_from_study_edit(net: &Network, edit: &StudyEdit) -> Result<NetworkEdit, String> {
    match edit {
        StudyEdit::DemandDelta { bus, p_mw, q_mvar } => {
            if q_mvar.is_some() {
                return Err(
                    "demand delta carries a reactive component, which tellegen does not model"
                        .into(),
                );
            }
            Ok(NetworkEdit::AddLoad {
                bus: key_from_ref(net, bus, "buses")?,
                p_mw: *p_mw,
            })
        }
        StudyEdit::RatingDelta { branch, delta_mw } => Ok(NetworkEdit::AdjustBranchRating {
            branch: key_from_ref(net, branch, "branches")?,
            delta_mw: *delta_mw,
        }),
        StudyEdit::SetFields { .. } => {
            Err("study contains a set_fields edit, which tellegen does not model".into())
        }
        StudyEdit::Unknown { kind, .. } => {
            Err(format!("study contains an unsupported edit kind `{kind}`"))
        }
        // `StudyEdit` is `#[non_exhaustive]`: a future kind rejects rather than
        // silently dropping.
        _ => Err("study contains an edit kind this build does not understand".into()),
    }
}

/// The row uid for a bus edit key on the stamped base network. A numeric id names the
/// bus's `id` field; a uid names the row directly. Both must resolve.
fn bus_uid_for_key(net: &Network, key: &ElementKey) -> Result<String, String> {
    let bus = match key {
        ElementKey::Id(id) => {
            let id = usize::try_from(*id)
                .map_err(|_| format!("demand edit bus id {id} out of range"))?;
            net.buses.iter().find(|b| b.id.0 == id)
        }
        ElementKey::Uid(uid) => net.buses.iter().find(|b| b.uid.as_deref() == Some(uid)),
    };
    bus.and_then(|b| b.uid.clone())
        .ok_or_else(|| format!("demand edit names bus {key}, which is not in the network"))
}

/// The numeric bus id for a bus edit key. A numeric key is the id itself; a uid
/// resolves to its row's `id` field. Errors on an unresolved uid.
fn bus_id_for_key(net: &Network, key: &ElementKey) -> Result<i64, String> {
    match key {
        ElementKey::Id(id) => Ok(*id),
        ElementKey::Uid(uid) => net
            .buses
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
fn branch_id_for_key(net: &Network, key: &ElementKey) -> Result<i64, String> {
    match key {
        ElementKey::Id(id) => Ok(*id),
        ElementKey::Uid(uid) => net
            .branches
            .iter()
            .position(|br| br.uid.as_deref() == Some(uid))
            .map(|pos| (pos + 1) as i64)
            .ok_or_else(|| {
                format!("rating edit names branch uid \"{uid}\", which is not in the network")
            }),
    }
}

/// The row uid for a branch edit key on the stamped base network. A numeric id is the
/// 1-based branch position; a uid names the row directly.
fn branch_uid_for_key(net: &Network, key: &ElementKey) -> Result<String, String> {
    let branch = match key {
        ElementKey::Id(id) => usize::try_from(*id)
            .ok()
            .filter(|&pos| pos >= 1)
            .and_then(|pos| net.branches.get(pos - 1)),
        ElementKey::Uid(uid) => net
            .branches
            .iter()
            .find(|br| br.uid.as_deref() == Some(uid)),
    };
    branch
        .and_then(|br| br.uid.clone())
        .ok_or_else(|| format!("rating edit names branch {key}, which is not in the network"))
}

/// Recover a tellegen [`ElementKey`] from a package [`ElementRef`] on `table`. Prefers
/// the row's payload identity (`source_uid`); falls back to the row's stamped uid when
/// only a wire row is given. Rejects a ref on the wrong table or one that resolves to
/// no uid.
fn key_from_ref(net: &Network, r: &ElementRef, table: &str) -> Result<ElementKey, String> {
    if r.table != table {
        return Err(format!(
            "study edit reference names table `{}`, expected `{table}`",
            r.table
        ));
    }
    if let Some(uid) = &r.source_uid {
        return Ok(ElementKey::Uid(uid.clone()));
    }
    let row = r
        .row
        .ok_or("study edit reference has neither source_uid nor row")?;
    let uid = if table == "buses" {
        net.buses.get(row).and_then(|b| b.uid.clone())
    } else {
        net.branches.get(row).and_then(|br| br.uid.clone())
    };
    uid.map(ElementKey::Uid)
        .ok_or_else(|| format!("study edit reference row {row} on `{table}` has no uid"))
}

/// Export the balanced study state at commit `commit` through a powerio format writer.
///
/// Parses a `.pio.json` package, materializes commits `0..=commit` onto the payload via
/// powerio-pkg (or writes the base payload when the package carries no study commits),
/// and serializes the result to `format` (`matpower`, `psse`, `powerio-json`, ...). Any
/// fidelity the target format cannot carry rides back in `warnings` so the caller can
/// surface it. Untrusted input: malformed, truncated, or wrong-shaped JSON returns a
/// typed error, never a panic.
pub fn export_study(
    package_json: &str,
    commit: usize,
    format: &str,
) -> Result<ExportedCase, String> {
    let package = NetworkPackage::from_json(package_json)
        .map_err(|e| format!("invalid .pio.json package: {e}"))?;
    let target = powerio::target_format_from_name(format)
        .ok_or_else(|| format!("unknown export format \"{format}\""))?;
    let balanced = match package.study() {
        Some(study) if !study.commits.is_empty() => package
            .materialize_balanced_study_commit(commit)
            .map_err(|e| format!("materialize study commit {commit}: {e}"))?
            .ok_or("package payload is not balanced")?,
        _ => package
            .as_balanced()
            .cloned()
            .ok_or("package payload is not balanced")?,
    };
    let conversion = powerio::write_as(&balanced, target).map_err(|e| e.to_string())?;
    Ok(ExportedCase {
        text: conversion.text,
        warnings: conversion.warnings,
        format: target.token().to_owned(),
        extension: target.extension().to_owned(),
    })
}

/// A study state written to a target format: the serialized case text, the writer's
/// fidelity warnings (empty for a faithful conversion), and the format token and file
/// extension so a caller can name the download.
#[derive(Clone, Debug, Serialize)]
pub struct ExportedCase {
    pub text: String,
    pub warnings: Vec<String>,
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
    let mut cols = Vec::new();
    let mut col_mag = Vec::new();
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
            cols.push(i);
            col_mag.push(m);
        }
    }
    (cols, col_mag)
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

    fn case3_json() -> String {
        powerio::parse_str(crate::model::CASE3, "matpower")
            .expect("parse")
            .network
            .to_json()
            .expect("to_json")
    }

    #[test]
    fn commit_matches_solve_json() {
        // A Study commit is the stateful face of the same driver: the response is
        // byte-identical to the stateless solve_json at the same operating point.
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let resp = s
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 50.0,
                }],
                SolveOptions::default(),
            )
            .expect("commit");
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}}}"#,
        )
        .expect("solve_json");
        assert_eq!(from_study, stateless);
    }

    #[test]
    fn commit_with_sensitivities_matches_solve_json() {
        // A commit that carries a Price/Demand cell returns the sensitivity column in the
        // same solve, byte-equal to the same stateless solve_json request — so the
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
                SolveOptions::default(),
            )
            .expect("commit_with");
        // The cell is present in the committed response (no second solve needed).
        assert_eq!(resp.sensitivities.len(), 1);
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}},"sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}]}"#,
        )
        .expect("solve_json");
        assert_eq!(from_study, stateless);
    }

    /// CASE3 with powerio row uids stamped by hand, in the `ensure_payload_uids`
    /// scheme; see the api.rs fixture of the same name.
    fn case3_with_uids_json() -> String {
        let mut net = powerio::parse_str(crate::model::CASE3, "matpower")
            .expect("parse")
            .network;
        for (i, b) in net.buses.iter_mut().enumerate() {
            b.uid = Some(format!("buses:{i}"));
        }
        for (i, br) in net.branches.iter_mut().enumerate() {
            br.uid = Some(format!("branches:{i}"));
        }
        net.to_json().expect("to_json")
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

        let r_id = by_id.commit(&[id_edit], SolveOptions::default()).unwrap();
        let r_uid = by_uid.commit(&[uid_edit], SolveOptions::default()).unwrap();
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
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: ElementKey::Uid("buses:99".into()),
                    p_mw: 10.0,
                }],
                SolveOptions::default(),
            )
            .unwrap_err();
        assert!(
            err.contains(r#"unknown demand delta bus "buses:99""#),
            "got: {err}"
        );
        assert_eq!(before, serde_json::to_string(s.solution()).unwrap());
        assert!(s.edits().is_empty());
    }

    #[test]
    fn edits_accumulate_across_commits() {
        let net = case3_json();
        let mut a = Study::new(&net, Problem::DcOpf).unwrap();
        a.commit(
            &[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let two = a
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 20.0,
                }],
                SolveOptions::default(),
            )
            .unwrap();
        assert_eq!(a.edits().len(), 2);
        // Two commits of +30 then +20 reach the same point as one +50.
        let mut b = Study::new(&net, Problem::DcOpf).unwrap();
        let once = b
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 50.0,
                }],
                SolveOptions::default(),
            )
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
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let replaced = s
            .replace_edits(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 50.0,
                }],
                SolveOptions::default(),
            )
            .unwrap();

        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}}}"#,
        )
        .expect("solve_json");
        assert_eq!(serde_json::to_string(&replaced).unwrap(), stateless);
        assert_eq!(s.edits().len(), 1);
    }

    #[test]
    fn replace_edits_can_reset_to_base() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let reset = s.replace_edits(&[], SolveOptions::default()).unwrap();
        let base = crate::solve_json(&net, r#"{"formulation":"dcopf"}"#).expect("solve_json");
        assert_eq!(serde_json::to_string(&reset).unwrap(), base);
        assert!(s.edits().is_empty());
    }

    #[test]
    fn failed_replace_keeps_last_committed_point() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.replace_edits(
            &[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 30.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let committed = serde_json::to_string(s.solution()).unwrap();

        let err = s
            .replace_edits(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 1_000_000.0,
                }],
                SolveOptions::default(),
            )
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
            .replace_edits(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 30.0,
                }],
                SolveOptions::default(),
            )
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
            .replace_edits(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 30.0,
                }],
                SolveOptions::default(),
            )
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
        assert_eq!(prev.operands[0].units, "$/MWh");

        let base: Value =
            serde_json::from_str(&serde_json::to_string(study.solution()).unwrap()).unwrap();
        let mut committed_study = Study::new(&net, Problem::DcOpf).unwrap();
        let committed = committed_study
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: step,
                }],
                SolveOptions::default(),
            )
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
    fn commit_with_rating_edit_matches_solve_json() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).expect("study");
        let resp = s
            .commit(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -210.0,
                }],
                SolveOptions::default(),
            )
            .expect("commit");
        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"dcopf","edits":{"rates":{"3":-210.0}}}"#,
        )
        .expect("solve_json");
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
            .replace_edits(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -210.0,
                }],
                SolveOptions::default(),
            )
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
        assert_eq!(prev.operands[0].units, "$/MWh");

        let committed: Value =
            serde_json::from_str(&serde_json::to_string(study.solution()).unwrap()).unwrap();
        let mut exact_study = Study::new(&net, Problem::DcOpf).unwrap();
        let exact = exact_study
            .replace_edits(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -210.0 + step,
                }],
                SolveOptions::default(),
            )
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
    fn mixed_demand_and_rating_edits_fold_replay_and_preview() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        let resp = s
            .commit(
                &[
                    NetworkEdit::AddLoad {
                        bus: 2.into(),
                        p_mw: 20.0,
                    },
                    NetworkEdit::AdjustBranchRating {
                        branch: 3.into(),
                        delta_mw: -210.0,
                    },
                ],
                SolveOptions::default(),
            )
            .unwrap();
        let stateless = crate::solve_json(
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

        // Resetting a mixed log replays to the base solve (replacement_step emits
        // both edit kinds).
        let reset = s.replace_edits(&[], SolveOptions::default()).unwrap();
        let base = crate::solve_json(&net, r#"{"formulation":"dcopf"}"#).unwrap();
        assert_eq!(serde_json::to_string(&reset).unwrap(), base);
        assert!(s.edits().is_empty());
    }

    #[test]
    fn rating_edit_validation_errors() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        let err = s
            .commit(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 99.into(),
                    delta_mw: -10.0,
                }],
                SolveOptions::default(),
            )
            .unwrap_err();
        assert!(err.contains("unknown rating delta branch 99"), "got: {err}");
        let err = s
            .commit(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -250.0,
                }],
                SolveOptions::default(),
            )
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
            .commit(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -10.0,
                }],
                SolveOptions::default(),
            )
            .unwrap_err();
        assert!(
            err.contains("branch rating edits are not supported by acpf"),
            "got: {err}"
        );
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_commit_with_rating_edit_matches_solve_json() {
        let net = case3_json();
        let mut s = Study::new(&net, Problem::Socwr).expect("socwr study");
        let resp = s
            .commit(
                &[NetworkEdit::AdjustBranchRating {
                    branch: 3.into(),
                    delta_mw: -210.0,
                }],
                SolveOptions::default(),
            )
            .expect("commit");
        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"socwr","edits":{"rates":{"3":-210.0}}}"#,
        )
        .expect("solve_json");
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
        // commit is byte-equal to the same stateless solve_json request.
        let net = case3_json();
        let mut s = Study::new(&net, Problem::Socwr).expect("socwr study");
        assert_eq!(s.formulation(), Problem::Socwr);
        let resp = s
            .commit(
                &[NetworkEdit::AddLoad {
                    bus: 2.into(),
                    p_mw: 10.0,
                }],
                SolveOptions::default(),
            )
            .expect("socwr commit");
        assert!(resp.w.is_some(), "socwr reports w");
        let from_study = serde_json::to_string(&resp).unwrap();
        let stateless = crate::solve_json(
            &net,
            r#"{"formulation":"socwr","edits":{"deltas":{"2":10.0}}}"#,
        )
        .expect("solve_json");
        assert_eq!(from_study, stateless);
    }

    #[cfg(not(feature = "conic"))]
    #[test]
    fn study_rejects_unbuilt_formulation() {
        // Without the conic feature SOCWR is not in this build; the error names solve_json
        // as the stateless route.
        let err = Study::new(&case3_json(), Problem::Socwr).unwrap_err();
        assert!(err.contains("does not support"), "got: {err}");
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
    // Package save / load / export
    // -----------------------------------------------------------------------

    fn dcopf_objective(network_json: &str) -> f64 {
        let out = crate::solve_json(network_json, r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        v["objective"].as_f64().unwrap()
    }

    /// A minimal balanced package on CASE3 carrying `commits` and an optional
    /// `app["tellegen"]` blob, for fail-closed load tests.
    fn package_with(commits: Vec<Vec<StudyEdit>>, app: Option<Value>) -> NetworkPackage {
        let mut net = powerio::parse_str(crate::model::CASE3, "matpower")
            .expect("parse")
            .network;
        powerio_pkg::ensure_payload_uids(&mut net);
        let mut study = StudyBlock::default();
        study.commits = commits
            .into_iter()
            .map(|edits| {
                let mut c = StudyCommit::default();
                c.edits = edits;
                c
            })
            .collect();
        if let Some(app) = app {
            study.app.insert("tellegen".to_owned(), app);
        }
        let mut package = NetworkPackage::from_balanced(net);
        package.set_study(study);
        package
    }

    fn valid_app() -> Value {
        serde_json::json!({
            "schema_version": 1,
            "formulation": "dcopf",
            "options": { "shed": false, "warm_start": true }
        })
    }

    #[test]
    fn save_load_round_trip_preserves_solution_and_log() {
        // A uid-keyed study of a demand edit and a rating edit saves to a package and
        // loads back to the same committed solution and the same edit log, edit by edit.
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(
            &[
                NetworkEdit::AddLoad {
                    bus: ElementKey::Uid("buses:1".into()),
                    p_mw: 20.0,
                },
                NetworkEdit::AdjustBranchRating {
                    branch: ElementKey::Uid("branches:2".into()),
                    delta_mw: -210.0,
                },
            ],
            SolveOptions::default(),
        )
        .unwrap();

        let package = s.to_package().unwrap();
        let restored = Study::from_package(&package).unwrap();

        assert_eq!(restored.formulation(), Problem::DcOpf);
        assert_eq!(restored.commits(), s.commits());
        assert_eq!(
            serde_json::to_string(restored.solution()).unwrap(),
            serde_json::to_string(s.solution()).unwrap()
        );
        assert_eq!(
            serde_json::to_string(restored.edits()).unwrap(),
            serde_json::to_string(s.edits()).unwrap()
        );

        // The round trip also survives serialization to `.pio.json` text.
        let text = package.to_json().unwrap();
        let reparsed = NetworkPackage::from_json(&text).unwrap();
        let restored2 = Study::from_package(&reparsed).unwrap();
        assert_eq!(
            serde_json::to_string(restored2.solution()).unwrap(),
            serde_json::to_string(s.solution()).unwrap()
        );
    }

    #[test]
    fn id_keyed_edits_round_trip_to_the_same_solution() {
        // An id-keyed edit persists by the row's uid, so the reloaded log is uid-keyed;
        // it names the same element and reaches the same committed solution.
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: 2.into(),
                p_mw: 50.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let restored = Study::from_package(&s.to_package().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_string(restored.solution()).unwrap(),
            serde_json::to_string(s.solution()).unwrap()
        );
        // Bus id 2 is row 1: the persisted key is the row uid.
        let restored_edits = serde_json::to_string(restored.edits()).unwrap();
        assert!(restored_edits.contains("buses:1"), "got: {restored_edits}");
    }

    #[test]
    fn one_study_commit_per_commit_call() {
        // Two commit calls become two package study commits; materializing commit 0
        // holds only the first batch, commit 1 holds both.
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 30.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 20.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        assert_eq!(s.commits(), 2);
        let package = s.to_package().unwrap();
        assert_eq!(package.study().unwrap().commits.len(), 2);

        let json = package.to_json().unwrap();
        let at0 = export_study(&json, 0, "powerio-json").unwrap();
        let at1 = export_study(&json, 1, "powerio-json").unwrap();
        // Commit 0 is base + 30 MW; commit 1 is base + 50 MW: strictly higher cost.
        let obj0 = dcopf_objective(&at0.text);
        let obj1 = dcopf_objective(&at1.text);
        assert!(
            obj1 > obj0,
            "commit 1 ({obj1}) should cost more than commit 0 ({obj0})"
        );
        // Commit 1 reproduces the study's committed objective.
        assert!((obj1 - s.solution().objective.unwrap()).abs() < 1e-6);
    }

    #[test]
    fn export_at_commit_reparses_and_solves_to_the_same_objective() {
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::DcOpf).unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 50.0,
            }],
            SolveOptions::default(),
        )
        .unwrap();
        let committed = s.solution().objective.unwrap();
        let json = s.to_package().unwrap().to_json().unwrap();
        let commit = s.commits() - 1;

        // The exact snapshot format re-solves to the identical objective.
        let pio = export_study(&json, commit, "powerio-json").unwrap();
        assert!((dcopf_objective(&pio.text) - committed).abs() < 1e-9);

        // MATPOWER round-trips the folded model too (to solver tolerance).
        let m = export_study(&json, commit, "matpower").unwrap();
        assert_eq!(m.extension, "m");
        let reparsed = powerio::parse_str(&m.text, "matpower")
            .unwrap()
            .network
            .to_json()
            .unwrap();
        assert!((dcopf_objective(&reparsed) - committed).abs() < 1e-3);
    }

    #[test]
    fn export_with_no_study_commits_writes_the_base_case() {
        let net = case3_json();
        let s = Study::new(&net, Problem::DcOpf).unwrap();
        let json = s.to_package().unwrap().to_json().unwrap();
        let base = export_study(&json, 0, "powerio-json").unwrap();
        assert!((dcopf_objective(&base.text) - s.solution().objective.unwrap()).abs() < 1e-9);
    }

    #[test]
    fn load_rejects_non_balanced_payload() {
        // A study package must be balanced; a multiconductor payload rejects on load.
        let package =
            NetworkPackage::from_multiconductor(powerio_dist::MulticonductorNetwork::default());
        let err = Study::from_package(&package).unwrap_err();
        assert!(err.contains("not balanced"), "got: {err}");
    }

    #[test]
    fn load_rejects_unknown_edit_kind() {
        let package = package_with(
            vec![vec![StudyEdit::Unknown {
                kind: "teleport".into(),
                value: serde_json::json!({ "kind": "teleport" }),
            }]],
            Some(valid_app()),
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(
            err.contains("unsupported edit kind `teleport`"),
            "got: {err}"
        );
    }

    #[test]
    fn load_rejects_set_fields_edit() {
        let update = powerio_pkg::ElementUpdate::new(
            ElementRef::by_source_uid("buses", "buses:0"),
            std::collections::BTreeMap::new(),
        );
        let package = package_with(
            vec![vec![StudyEdit::SetFields { update }]],
            Some(valid_app()),
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(err.contains("set_fields"), "got: {err}");
    }

    #[test]
    fn load_rejects_missing_app_metadata() {
        let package = package_with(
            vec![vec![StudyEdit::DemandDelta {
                bus: ElementRef::by_source_uid("buses", "buses:1"),
                p_mw: 10.0,
                q_mvar: None,
            }]],
            None,
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(err.contains("app[\"tellegen\"]"), "got: {err}");
    }

    #[test]
    fn load_rejects_wrong_app_version() {
        let mut app = valid_app();
        app["schema_version"] = serde_json::json!(999);
        let package = package_with(
            vec![vec![StudyEdit::DemandDelta {
                bus: ElementRef::by_source_uid("buses", "buses:1"),
                p_mw: 10.0,
                q_mvar: None,
            }]],
            Some(app),
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(err.contains("schema_version 999"), "got: {err}");
    }

    #[test]
    fn load_rejects_unresolved_edit_key() {
        let package = package_with(
            vec![vec![StudyEdit::DemandDelta {
                bus: ElementRef::by_source_uid("buses", "buses:99"),
                p_mw: 10.0,
                q_mvar: None,
            }]],
            Some(valid_app()),
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(
            err.contains("unknown demand delta bus") && err.contains("buses:99"),
            "got: {err}"
        );
    }

    #[test]
    fn load_rejects_reactive_demand_delta() {
        let package = package_with(
            vec![vec![StudyEdit::DemandDelta {
                bus: ElementRef::by_source_uid("buses", "buses:1"),
                p_mw: 10.0,
                q_mvar: Some(5.0),
            }]],
            Some(valid_app()),
        );
        let err = Study::from_package(&package).unwrap_err();
        assert!(err.contains("reactive"), "got: {err}");
    }

    #[test]
    fn export_rejects_malformed_package_json() {
        for bad in ["", "{", "not json", "[]", "null", "{\"schema\":\"x\"}"] {
            let err = export_study(bad, 0, "matpower").unwrap_err();
            assert!(!err.is_empty(), "expected an error for {bad:?}");
        }
    }

    #[test]
    fn export_rejects_unknown_format() {
        let s = Study::new(&case3_json(), Problem::DcOpf).unwrap();
        let json = s.to_package().unwrap().to_json().unwrap();
        let err = export_study(&json, 0, "nonesuch").unwrap_err();
        assert!(err.contains("unknown export format"), "got: {err}");
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_study_round_trips_with_its_formulation_and_options() {
        let net = case3_with_uids_json();
        let mut s = Study::new(&net, Problem::Socwr).unwrap();
        s.commit(
            &[NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 10.0,
            }],
            SolveOptions {
                shed: true,
                warm_start: false,
            },
        )
        .unwrap();
        let restored = Study::from_package(&s.to_package().unwrap()).unwrap();
        assert_eq!(restored.formulation(), Problem::Socwr);
        assert_eq!(
            serde_json::to_string(restored.solution()).unwrap(),
            serde_json::to_string(s.solution()).unwrap()
        );
    }
}
