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
//! v1 implements the continuous active-demand drag ([`NetworkEdit::AddLoad`]) over the
//! two default-feature formulations — DC OPF and AC power flow. The enum and the solved
//! state are `#[non_exhaustive]`/extensible: topology edits, other-parameter edits, and
//! the conic / AC-OPF formulations slot into the same shape. SOCWR and AC OPF (and any
//! formulation this build does not include) return a clean error; use `solve_json` for
//! stateless solves of those.

use std::collections::HashMap;

use powerio::network::Network;
use serde::{Deserialize, Serialize};

use crate::api::{
    acpf_assemble, acpf_solved, dcopf_assemble, dcopf_solved, run_cells, Edits, Problem,
    SensRequest, SolveOptions, SolveRequest, SolveResponse,
};
use crate::model::{AcNetwork, DcNetwork};
use crate::problem::AcPfSolution;
use crate::sens::{AcNewton, DcKkt, Differentiable, ElementId, Mode, Operand, Parameter, Power};
use crate::solve::DcSolution;

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

/// The committed solved state, retained so [`preview`](Study::preview) can build the
/// formulation's differentiable system without re-solving.
enum Solved {
    Dc(DcNetwork, DcSolution),
    AcPf(AcNetwork, AcPfSolution),
}

/// A stateful, build-once handle. Construct with a network and a formulation; the base
/// model is built once and the base case solved immediately, so [`solution`](Study::solution)
/// and [`preview`](Study::preview) are available right away.
pub struct Study {
    formulation: Problem,
    base_dc: Option<DcNetwork>,
    base_ac: Option<AcNetwork>,
    options: SolveOptions,
    log: Vec<NetworkEdit>,
    solved: Solved,
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
        let req = bare_request(formulation, &options);
        match formulation {
            Problem::DcOpf => {
                let base = DcNetwork::from_network(net)?;
                let (dc, sol) = dcopf_solved(base.clone(), &req, None)?;
                let last = dcopf_assemble(&dc, &sol, &req)?;
                Ok(Study {
                    formulation,
                    base_dc: Some(base),
                    base_ac: None,
                    options,
                    log: Vec::new(),
                    solved: Solved::Dc(dc, sol),
                    last,
                })
            }
            Problem::AcPf => {
                let base = AcNetwork::from_network(net)?;
                let (ac, sol) = acpf_solved(base.clone(), &req)?;
                let last = acpf_assemble(&ac, &sol, &req)?;
                Ok(Study {
                    formulation,
                    base_dc: None,
                    base_ac: Some(base),
                    options,
                    log: Vec::new(),
                    solved: Solved::AcPf(ac, sol),
                    last,
                })
            }
            other => Err(format!(
                "Study does not yet support {other:?}; use solve_json for stateless {other:?} solves"
            )),
        }
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

    /// Apply `edits` to the committed operating point and exact-re-solve. This is the
    /// source of truth; the new solution becomes the committed point. The base model is
    /// reused (cloned and perturbed), so this never re-parses the network.
    pub fn commit(
        &mut self,
        edits: &[NetworkEdit],
        options: SolveOptions,
    ) -> Result<SolveResponse, String> {
        self.options = options;
        self.log.extend_from_slice(edits);
        self.resolve()
    }

    fn resolve(&mut self) -> Result<SolveResponse, String> {
        let req = SolveRequest {
            formulation: self.formulation,
            edits: fold(&self.log),
            options: self.options.clone(),
            ..Default::default()
        };
        match self.formulation {
            Problem::DcOpf => {
                let base = self.base_dc.clone().expect("dc base present for DcOpf");
                let (dc, sol) = dcopf_solved(base, &req, None)?;
                let resp = dcopf_assemble(&dc, &sol, &req)?;
                self.solved = Solved::Dc(dc, sol);
                self.last = resp.clone();
                Ok(resp)
            }
            Problem::AcPf => {
                let base = self.base_ac.clone().expect("ac base present for AcPf");
                let (ac, sol) = acpf_solved(base, &req)?;
                let resp = acpf_assemble(&ac, &sol, &req)?;
                self.solved = Solved::AcPf(ac, sol);
                self.last = resp.clone();
                Ok(resp)
            }
            // from_network only constructs DcOpf / AcPf studys.
            _ => unreachable!("study formulation is dcopf or acpf"),
        }
    }

    /// First-order prediction of applying `edits` at the committed point, for each
    /// `watched` operand, without re-solving. Reuses the committed solution's
    /// differentiable system: the `dz/dp` column dotted with the demand step. The result
    /// is a local linearization (`local_only = true`); `commit` to confirm.
    pub fn preview(&self, edits: &[NetworkEdit], watched: &[Operand]) -> Result<Preview, String> {
        // Transient demand step (MW) per original bus id.
        let mut mag: HashMap<i64, f64> = HashMap::new();
        for e in edits {
            *mag.entry(e.bus()).or_insert(0.0) += e.p_mw();
        }

        match &self.solved {
            Solved::Dc(dc, sol) => {
                let (cols, col_mag) = dense_cols(&dc.bus_ids, &mag);
                let operands = preview_columns(&DcKkt::new(dc, sol), &cols, &col_mag, watched)?;
                // ∂objective/∂demand is the committed marginal price: Δobj ≈ Σ lmp_b · Δp_b.
                let lmp = sol.lmp_usd_per_mwh(dc.base_mva);
                let objective_delta = cols.iter().zip(&col_mag).map(|(&i, &m)| lmp[i] * m).sum();
                Ok(Preview {
                    operands,
                    objective_delta: Some(objective_delta),
                    local_only: true,
                })
            }
            Solved::AcPf(ac, sol) => {
                let (cols, col_mag) = dense_cols(&ac.bus_ids, &mag);
                let operands = preview_columns(&AcNewton::new(ac, sol), &cols, &col_mag, watched)?;
                // AC power flow has no objective.
                Ok(Preview {
                    operands,
                    objective_delta: None,
                    local_only: true,
                })
            }
        }
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

fn bare_request(formulation: Problem, options: &SolveOptions) -> SolveRequest {
    SolveRequest {
        formulation,
        edits: Edits::default(),
        options: options.clone(),
        ..Default::default()
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

/// For each watched operand, run the demand sensitivity over the edited buses and dot it
/// with the demand step to get the predicted operand change (in served units).
fn preview_columns(
    sys: &dyn Differentiable,
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
            parameter: Parameter::Demand(Power::Active),
            indices: Some(cols.to_vec()),
            mode: Mode::Auto,
        })
        .collect();
    let mats = run_cells(sys, &reqs)?;

    Ok(watched
        .iter()
        .zip(mats)
        .map(|(&operand, m)| {
            // values[r][c] = d(operand_r)/d(demand at cols[c]); the column order matches
            // col_mag, so the predicted change is the row dotted with the demand step.
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
    fn study_rejects_unsupported_formulation() {
        // SOCWR / AC OPF (and anything else) are not wired into the Study yet; the
        // error names solve_json as the stateless route.
        let err = Study::new(&case3_json(), Problem::Socwr).unwrap_err();
        assert!(err.contains("does not yet support"), "got: {err}");
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
