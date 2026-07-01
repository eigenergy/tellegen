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
use serde::{Deserialize, Serialize};

use crate::api::{
    ac_pf_assemble, ac_pf_solved, dc_opf_assemble, dc_opf_solved, run_cells, Edits, Problem,
    SensRequest, SolveOptions, SolveRequest, SolveResponse,
};
use crate::model::{AcNetwork, DcNetwork};
use crate::problem::AcPfSolution;
use crate::sens::{AcNewton, DcKkt, Differentiable, ElementId, Mode, Operand, Parameter, Power};
use crate::solve::DcSolution;

#[cfg(feature = "conic")]
use crate::api::{socwr_assemble, socwr_solved};
#[cfg(feature = "conic")]
use crate::problem::SocWrSolution;
#[cfg(feature = "conic")]
use crate::sens::ConicKkt;

/// A typed edit to the operating point. v1: the continuous active-demand drag. The enum
/// is `#[non_exhaustive]` and serde-tagged (`{"kind":"add_load","bus":2,"p_mw":50}`), so
/// topology and other-parameter edits extend the wire format without breaking a client
/// that knows only the demand edit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum NetworkEdit {
    /// Add `p_mw` to the active demand at the bus with this original id. Repeated edits
    /// accumulate; the committed operating point is the base case plus the whole log.
    AddLoad { bus: i64, p_mw: f64 },
}

impl NetworkEdit {
    fn bus(&self) -> i64 {
        match self {
            NetworkEdit::AddLoad { bus, .. } => *bus,
        }
    }
    fn p_mw(&self) -> f64 {
        match self {
            NetworkEdit::AddLoad { p_mw, .. } => *p_mw,
        }
    }

    /// The [`Parameter`] this edit perturbs, so [`preview`](Study::preview) differentiates
    /// the watched operands with respect to the right axis. The active-demand drag maps to
    /// `Demand(Active)`; new edit kinds add their own arm here.
    fn parameter(&self) -> Parameter {
        match self {
            NetworkEdit::AddLoad { .. } => Parameter::Demand(Power::Active),
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
}

/// DC OPF committed state.
struct DcState {
    net: DcNetwork,
    sol: DcSolution,
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
        self.commit_log(next_log, sensitivities, options)
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
        self.commit_log(edits.to_vec(), sensitivities, options)
    }

    fn commit_log(
        &mut self,
        log: Vec<NetworkEdit>,
        sensitivities: &[SensRequest],
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        let (solved, resp) = self.solve_log(&log, sensitivities, &options)?;
        self.options = options;
        self.log = log;
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
        // Transient step magnitude per original bus id, and the parameter the edits
        // perturb. v1 has one edit kind, so the parameter is uniform; assert it.
        let mut mag: HashMap<i64, f64> = HashMap::new();
        let mut parameter: Option<Parameter> = None;
        for e in edits {
            *mag.entry(e.bus()).or_insert(0.0) += e.p_mw();
            match parameter {
                None => parameter = Some(e.parameter()),
                Some(p) if p == e.parameter() => {}
                Some(_) => {
                    return Err(
                        "preview cannot mix edit kinds that map to different parameters".into(),
                    )
                }
            }
        }
        // Default to the demand parameter when there is no edit (every column is zero).
        let parameter = parameter.unwrap_or(Parameter::Demand(Power::Active));

        let bus_ids = response_bus_ids(&self.last);
        let (cols, col_mag) = dense_cols(&bus_ids, &mag);

        let operands = self
            .solved
            .with_system(&mut |sys| preview_columns(sys, parameter, &cols, &col_mag, watched))?;

        // ∂objective/∂parameter is the committed marginal price dotted with the step, when
        // the formulation has an objective: Δobj ≈ Σ lmp_b · Δp_b. `None` for power flow.
        let objective_delta = self.solved.lmp().map(|lmp| {
            cols.iter()
                .zip(&col_mag)
                .map(|(&i, &m)| lmp[i] * m)
                .sum::<f64>()
        });

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

/// Collapse the edit log to the cumulative demand-delta map the model builders consume.
fn fold(log: &[NetworkEdit]) -> Edits {
    let mut deltas: HashMap<i64, f64> = HashMap::new();
    for e in log {
        match e {
            NetworkEdit::AddLoad { bus, p_mw } => *deltas.entry(*bus).or_insert(0.0) += *p_mw,
        }
    }
    Edits { deltas }
}

/// Compute the incremental edits that move from `current` to `target`, both treated as
/// absolute edit states. v1 has only additive load edits, so the state is the folded
/// demand-delta map.
fn replacement_step(current: &[NetworkEdit], target: &[NetworkEdit]) -> Vec<NetworkEdit> {
    let current = fold(current).deltas;
    let target = fold(target).deltas;
    let mut diff: BTreeMap<i64, f64> = target.into_iter().collect();
    for (bus, mw) in current {
        *diff.entry(bus).or_default() -= mw;
    }
    diff.into_iter()
        .filter(|(_, p_mw)| *p_mw != 0.0)
        .map(|(bus, p_mw)| NetworkEdit::AddLoad { bus, p_mw })
        .collect()
}

/// The dense bus-id order of a committed solution, read off whichever per-bus block the
/// formulation populated (`lmp`/`vm`/`va`/`w`). The preview maps edited bus ids onto these
/// dense indices, so it must use the same order the committed sensitivity matrix reports.
fn response_bus_ids(resp: &SolveResponse) -> Vec<usize> {
    let scalars = resp
        .lmp
        .as_deref()
        .or(resp.vm.as_deref())
        .or(resp.va.as_deref())
        .or(resp.w.as_deref());
    if let Some(s) = scalars {
        return s.iter().map(|b| b.bus).collect();
    }
    if let Some(inj) = resp.injections.as_deref() {
        return inj.iter().map(|b| b.bus).collect();
    }
    Vec::new()
}

/// Map edited bus ids to dense indices with aligned magnitudes (MW), dropping ids that
/// are not in this case.
fn dense_cols(bus_ids: &[usize], mag: &HashMap<i64, f64>) -> (Vec<usize>, Vec<f64>) {
    let idx: HashMap<usize, usize> = bus_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let mut cols = Vec::new();
    let mut col_mag = Vec::new();
    for (&bus, &m) in mag {
        if bus > 0 {
            if let Some(&i) = idx.get(&(bus as usize)) {
                cols.push(i);
                col_mag.push(m);
            }
        }
    }
    (cols, col_mag)
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

/// The served unit of a predicted operand delta. The sensitivity is `(operand)/MW`
/// (differentiated w.r.t. active demand); the predicted value is already multiplied by
/// the MW step, so it carries the operand unit — strip the `/MW` denominator and parens.
fn operand_unit(ratio: &str) -> String {
    let s = ratio.strip_suffix("/MW").unwrap_or(ratio).trim();
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 50.0 }],
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 50.0 }],
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

    #[test]
    fn edits_accumulate_across_commits() {
        let net = case3_json();
        let mut a = Study::new(&net, Problem::DcOpf).unwrap();
        a.commit(
            &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
            SolveOptions::default(),
        )
        .unwrap();
        let two = a
            .commit(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 20.0 }],
                SolveOptions::default(),
            )
            .unwrap();
        assert_eq!(a.edits().len(), 2);
        // Two commits of +30 then +20 reach the same point as one +50.
        let mut b = Study::new(&net, Problem::DcOpf).unwrap();
        let once = b
            .commit(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 50.0 }],
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
            &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
            SolveOptions::default(),
        )
        .unwrap();
        let replaced = s
            .replace_edits(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 50.0 }],
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
            &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
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
            &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
            SolveOptions::default(),
        )
        .unwrap();
        let committed = serde_json::to_string(s.solution()).unwrap();

        let err = s
            .replace_edits(
                &[NetworkEdit::AddLoad {
                    bus: 2,
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
                SolveOptions::default(),
            )
            .unwrap();
        let toward_fifty = absolute
            .preview_replacement(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 50.0 }],
                &[Operand::Price(Power::Active)],
            )
            .unwrap();

        let mut incremental = Study::new(&net, Problem::DcOpf).unwrap();
        incremental
            .replace_edits(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 30.0 }],
                SolveOptions::default(),
            )
            .unwrap();
        let plus_twenty = incremental
            .preview(
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 20.0 }],
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: step }],
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: step }],
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 1.0 }],
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
                &[NetworkEdit::AddLoad { bus: 2, p_mw: 10.0 }],
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
}
