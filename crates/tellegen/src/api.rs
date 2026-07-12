//! The browser- and server-facing entry point: one driver over every formulation.
//!
//! Parse a powerio [`Network`], apply operating-point edits, solve the requested
//! formulation, attach any requested sensitivity cells, and serve a
//! formulation-agnostic response. The frontend picks three things in one request:
//! the **problem** it solves (`dcpf`/`dcopf`/`acpf`/`socwr`), the **operand** it
//! differentiates, and the **parameter** it differentiates with respect to. The same
//! physical vocabulary the [`sensitivity`] driver uses ([`Operand`]/[`Parameter`])
//! crosses the JSON edge unchanged.
//!
//! Keeping the JSON layer here (not behind `#[wasm_bindgen]`) makes it testable
//! natively; the wasm crate wraps [`solve_json`] and [`capabilities_json`].

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use powerio::network::Network;
use serde::{Deserialize, Serialize};

use super::model::DcNetwork;
use super::problem::dc_opf_cancellable;
use super::solve::SolveIteration;

#[cfg(feature = "sensitivity")]
use super::sens::{
    sensitivity, served_units_label, Bound, CostTerm, Differentiable, End, Mode, Operand,
    Parameter, Power, SensitivityMatrix, VoltageKind, GB,
};

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// Which problem to solve. The convex/power flow solve paths, as the lowercase JSON
/// tags `"dcpf"`/`"dcopf"`/`"acpf"`/`"socwr"`. A plain (not internally tagged) enum
/// so a request that omits it defaults to [`DcOpf`](Problem::DcOpf), and `{}` is a
/// valid base-case DC OPF request.
///
/// The `"acopf"` tag (full nonlinear AC OPF) is retained for wire-format stability
/// but is not solved by this build: [`capabilities_json`] reports it unavailable and
/// requesting it returns a clean `Err`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Problem {
    /// DC power flow: angles and flows at the fixed generator setpoints. No prices,
    /// no dispatch, no sensitivity.
    DcPf,
    /// DC OPF: the LMP / dispatch / flow workhorse. Differentiable via the DC KKT.
    #[default]
    DcOpf,
    /// AC polar Newton power flow. Voltages and nodal injections. Differentiable
    /// via the AC Newton system.
    AcPf,
    /// SOCWR (Jabr) conic relaxation of AC OPF. Differentiable via the conic KKT.
    Socwr,
    /// Full nonlinear AC OPF. Not available in this build (the dispatch errors
    /// cleanly); the tag is kept so the JSON contract stays stable.
    Acopf,
}

/// How an edit names its element: the original numeric id (bus id, 1-based branch
/// position) or the powerio row uid (`"buses:0"`, `"branches:1"`, or a source uid
/// where the format defines one, e.g. GOC3). Untagged on the wire — a JSON number
/// (or all-digit string, since JSON object keys are strings) reads as [`Id`](Self::Id),
/// any other string as [`Uid`](Self::Uid) — so existing numeric clients keep working
/// unchanged. Uid keys resolve against the uids the network carried when the model
/// was built (the wasm `ingest_case` stamps them via powerio's
/// `ensure_payload_uids`); a uid the network does not carry is an unknown-element
/// error.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElementKey {
    /// Original numeric id: bus id or 1-based branch position.
    Id(i64),
    /// powerio row uid, e.g. `"branches:1"`.
    Uid(String),
}

impl std::fmt::Display for ElementKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElementKey::Id(id) => write!(f, "{id}"),
            ElementKey::Uid(uid) => write!(f, "\"{uid}\""),
        }
    }
}

impl From<i64> for ElementKey {
    fn from(id: i64) -> Self {
        ElementKey::Id(id)
    }
}

impl Serialize for ElementKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // A numeric key serializes as a JSON number in value position (the
            // pre-uid `NetworkEdit` wire shape) and as its decimal string in map-key
            // position (the pre-uid `Edits` wire shape) — serde_json does the
            // key-position stringification.
            ElementKey::Id(id) => serializer.serialize_i64(*id),
            ElementKey::Uid(uid) => serializer.serialize_str(uid),
        }
    }
}

impl<'de> Deserialize<'de> for ElementKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;
        impl serde::de::Visitor<'_> for KeyVisitor {
            type Value = ElementKey;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an integer element id or a `table:row` uid string")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<ElementKey, E> {
                Ok(ElementKey::Id(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ElementKey, E> {
                i64::try_from(v)
                    .map(ElementKey::Id)
                    .map_err(|_| E::custom(format!("element id {v} out of range")))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ElementKey, E> {
                // JSON object keys arrive as strings, so an all-digit string is the
                // numeric-id wire form (`{"deltas":{"2":50}}`), never a uid.
                Ok(match v.parse::<i64>() {
                    Ok(id) => ElementKey::Id(id),
                    Err(_) => ElementKey::Uid(v.to_owned()),
                })
            }
        }
        deserializer.deserialize_any(KeyVisitor)
    }
}

/// Operating-point edits applied before the model is built: demand deltas in MW
/// keyed by bus (the operating point is `base demand + delta`) and branch rating
/// deltas in MW keyed by branch (the thermal limit is `base rating + delta`). Keys
/// are [`ElementKey`]s — the original numeric id or the powerio row uid. A struct
/// (not a bare map) so the structural-edit vocabulary (add line, add generator,
/// retune a parameter) can grow without breaking the wire format: a client that
/// knows only `deltas` keeps working.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Edits {
    /// Active-power demand delta in MW per bus key.
    #[serde(default)]
    pub deltas: HashMap<ElementKey, f64>,
    /// Thermal rating delta in MW per branch key. Supported by the DC OPF
    /// and SOCWR paths; the AC power flow has no flow limits and rejects them.
    #[serde(default)]
    pub rates: HashMap<ElementKey, f64>,
}

/// Solve options orthogonal to the formulation choice.
#[derive(Clone, Debug, Deserialize)]
pub struct SolveOptions {
    /// Permit load shedding on the DC paths. Default `false`: an unservable case
    /// reports infeasible (the published PGLib behavior). Ignored by AC/conic.
    #[serde(default)]
    pub shed: bool,
    /// Retained for wire-format stability; inert in this build (it gated the AC OPF
    /// warm start). Default `true`, ignored by every formulation this build solves.
    #[serde(default = "default_true")]
    pub warm_start: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions {
            shed: false,
            warm_start: true,
        }
    }
}

/// One requested sensitivity cell: an [`Operand`] differentiated with respect to a
/// [`Parameter`], over an optional parameter-index subset, in an optional direction.
/// The operand/parameter are the contract's serde-tagged enums verbatim
/// (`{"Price":"Active"}` / `{"Demand":"Active"}`).
#[cfg(feature = "sensitivity")]
#[derive(Clone, Debug, Deserialize)]
pub struct SensRequest {
    pub operand: Operand,
    pub parameter: Parameter,
    /// Dense parameter-column indices; `None` computes the whole axis.
    #[serde(default)]
    pub indices: Option<Vec<usize>>,
    /// Forward / Adjoint / Auto. `Auto` when omitted.
    #[serde(default = "default_mode")]
    pub mode: Mode,
}

#[cfg(feature = "sensitivity")]
fn default_mode() -> Mode {
    Mode::Auto
}

/// The one solve request: a formulation, an operating-point edit set, zero or more
/// sensitivity cells, and options. A bare `{"formulation":"acpf"}` (or even `{}`,
/// which defaults to DC OPF) is valid.
///
/// ```json
/// {
///   "formulation": "dcopf",
///   "edits": { "deltas": { "2": 50.0 }, "rates": { "3": -25.0 } },
///   "sensitivities": [
///     { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"}, "indices": [1] }
///   ],
///   "options": { "shed": false }
/// }
/// ```
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SolveRequest {
    #[serde(default)]
    pub formulation: Problem,
    #[serde(default)]
    pub edits: Edits,
    /// Zero or more sensitivity cells, computed against the solved system in request
    /// order. Ignored by a build without the `sensitivity` feature.
    #[cfg(feature = "sensitivity")]
    #[serde(default)]
    pub sensitivities: Vec<SensRequest>,
    #[serde(default)]
    pub options: SolveOptions,
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// A solve outcome that succeeded. A failed solve is the `Err` arm of [`solve_json`].
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SolveStatus {
    /// An OPF reached optimality.
    Optimal,
    /// A power flow converged to a feasible point.
    Feasible,
}

/// The convergence record. OPF paths carry the full interior-point trace (for the
/// solve-card sparkline); the AC power flow carries its Newton count and final
/// mismatch. Untagged: the OPF arm serializes to the same bare array the DC OPF
/// always returned.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Iterations {
    /// Interior-point iterate trace (dcopf / socwr).
    Ipm(Vec<SolveIteration>),
    /// Newton iteration count and final infinity-norm mismatch (acpf).
    Newton { count: usize, residual: f64 },
}

/// A scalar keyed by original bus id (LMP, voltage, angle, squared magnitude).
/// `uid` is the bus's powerio row uid when the solved network carried one, so an
/// overlay can re-key on stable identity instead of the positional id.
#[derive(Clone, Debug, Serialize)]
pub struct BusScalar {
    pub bus: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub value: f64,
}

/// A nodal net injection (MW / MVAr), keyed by original bus id (plus the row uid
/// when carried, as in [`BusScalar`]). The AC power flow solution is nodal, not
/// branch-resolved, so it reports these instead of flows.
#[derive(Clone, Debug, Serialize)]
pub struct BusInjection {
    pub bus: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub p: f64,
    pub q: f64,
}

/// Branch flows, keyed by original branch id (plus the row uid when carried, as in
/// [`BusScalar`]). `pf` (from-end active, MW) and `loading` (|S|/limit,
/// dimensionless) are present on every formulation that has flows; the reactive
/// and to-end legs are `None` on the DC paths.
#[derive(Clone, Debug, Serialize)]
pub struct BranchFlow {
    pub branch: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub pf: f64,
    pub loading: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qf: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qt: Option<f64>,
}

/// Generator dispatch, keyed by original generator id. `qg` is `None` on the DC paths.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct GenDispatch {
    pub gen: usize,
    pub pg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qg: Option<f64>,
}

/// The formulation-agnostic solve result. A superset: every block is optional, and
/// each formulation fills what it produces. Powers are MW/MVAr, prices $/MWh and
/// $/MVArh, angles radians, `vm` per unit, `w = |V|^2` per unit squared. Element ids
/// are the original bus/branch/generator ids, so the frontend joins straight onto
/// its case.
#[derive(Clone, Debug, Serialize)]
pub struct SolveResponse {
    /// The formulation that produced this, echoed for the client.
    pub formulation: Problem,
    pub status: SolveStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iterations: Option<Iterations>,
    /// Active nodal price (dcopf / socwr).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmp: Option<Vec<BusScalar>>,
    /// Reactive nodal price. Always `None` in this build; the field is retained for
    /// wire-format stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmp_q: Option<Vec<BusScalar>>,
    /// Voltage magnitude, per unit (acpf). SOCWR reports `w`, not `vm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm: Option<Vec<BusScalar>>,
    /// Voltage angle, radians (every path except socwr, which is W-space).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub va: Option<Vec<BusScalar>>,
    /// Squared voltage magnitude `w = |V|^2`, per unit squared (socwr). The conic
    /// relaxation does not guarantee a consistent angle, so it reports `w`, not `vm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<Vec<BusScalar>>,
    /// Nodal injections (acpf), MW/MVAr.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injections: Option<Vec<BusInjection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flows: Option<Vec<BranchFlow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<Vec<GenDispatch>>,
    /// One self-describing matrix per requested cell, in request order. Each carries
    /// its own row/column element ids and the served-unit label.
    #[cfg(feature = "sensitivity")]
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sensitivities: Vec<SensitivityMatrix>,
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// Parse a network, solve the requested formulation at `base + edits`, attach every
/// requested sensitivity cell, and return the [`SolveResponse`] as JSON. The single
/// front door. Errors — a failed solve, an unsupported `(operand, parameter)` cell,
/// a formulation this build does not include — surface as the `Err` string.
pub fn solve_json(network_json: &str, request_json: &str) -> Result<String, String> {
    let net = Network::from_json(network_json).map_err(|e| e.to_string())?;
    let req: SolveRequest = if request_json.trim().is_empty() {
        SolveRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|e| format!("bad request JSON: {e}"))?
    };
    let resp = solve_network(&net, &req)?;
    serde_json::to_string(&resp).map_err(|e| e.to_string())
}

/// Solve an already-parsed [`Network`] under `req`. Dispatches on the formulation to
/// the matching solver, then runs each requested sensitivity against the matching
/// differentiable system. Problems this build does not include return a clean
/// `Err` rather than degrading silently.
pub fn solve_network(net: &Network, req: &SolveRequest) -> Result<SolveResponse, String> {
    match req.formulation {
        Problem::DcOpf => solve_dc_opf(net, req),
        #[cfg(feature = "sensitivity")]
        Problem::DcPf => solve_dc_pf(net, req),
        #[cfg(not(feature = "sensitivity"))]
        Problem::DcPf => Err("dcpf requires the `sensitivity` feature".into()),
        #[cfg(feature = "sensitivity")]
        Problem::AcPf => solve_ac_pf(net, req),
        #[cfg(not(feature = "sensitivity"))]
        Problem::AcPf => Err("acpf requires the `sensitivity` feature".into()),
        #[cfg(feature = "conic")]
        Problem::Socwr => solve_socwr(net, req),
        #[cfg(not(feature = "conic"))]
        Problem::Socwr => Err("socwr requires the `conic` feature".into()),
        Problem::Acopf => {
            Err("acopf (full nonlinear AC OPF) is not available in this build".into())
        }
    }
}

fn solve_dc_opf(net: &Network, req: &SolveRequest) -> Result<SolveResponse, String> {
    let dc = DcNetwork::from_network(net)?;
    dc_opf_response(dc, req, None)
}

/// Apply the request's operating-point edits to an owned [`DcNetwork`] and solve the
/// DC OPF, returning the perturbed model alongside its solution. Kept separate from
/// [`dc_opf_assemble`] so a [`Study`](crate::study::Study) can retain the solved
/// model + solution and build a `DcKkt` for first-order previews without re-solving.
pub(crate) fn dc_opf_solved(
    mut dc: DcNetwork,
    req: &SolveRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(DcNetwork, super::problem::DcOpfSolution), String> {
    dc.allow_shed = req.options.shed;
    apply_demand_deltas(&mut dc, &req.edits.deltas)?;
    apply_rating_deltas(&mut dc, &req.edits.rates)?;
    let sol = dc_opf_cancellable(&dc, cancel)?;
    Ok((dc, sol))
}

/// Assemble the DC OPF [`SolveResponse`] (and any requested sensitivity cells) from a
/// solved model. Shared by the one-shot path and the cached [`Study`] path.
#[cfg_attr(not(feature = "sensitivity"), allow(unused_variables))]
pub(crate) fn dc_opf_assemble(
    dc: &DcNetwork,
    sol: &super::problem::DcOpfSolution,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    let base = dc.base_mva;
    let lmp = sol.lmp_usd_per_mwh(base);

    #[cfg(feature = "sensitivity")]
    let sensitivities = run_cells(&super::sens::DcKkt::new(dc, sol), &req.sensitivities)?;

    Ok(SolveResponse {
        formulation: Problem::DcOpf,
        status: SolveStatus::Optimal,
        objective: Some(sol.objective),
        iterations: Some(Iterations::Ipm(sol.iterations.clone())),
        lmp: Some(zip_bus(&dc.bus_ids, &dc.bus_uids, &lmp)),
        lmp_q: None,
        vm: None,
        va: Some(zip_bus(&dc.bus_ids, &dc.bus_uids, &sol.va)),
        w: None,
        injections: None,
        flows: Some(dc_branch_flows(
            &dc.branch_ids,
            &dc.branch_uids,
            &sol.f,
            &dc.fmax,
            base,
        )),
        dispatch: Some(zip_gen_pg(&dc.gen_ids, &sol.pg, base)),
        #[cfg(feature = "sensitivity")]
        sensitivities,
    })
}

/// Solve the DC OPF for an owned [`DcNetwork`] and assemble the response. Shared by
/// [`solve_dc_opf`] (build-then-solve) and [`solve_prebuilt`] (cached model).
fn dc_opf_response(
    dc: DcNetwork,
    req: &SolveRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<SolveResponse, String> {
    let (dc, sol) = dc_opf_solved(dc, req, cancel)?;
    dc_opf_assemble(&dc, &sol, req)
}

#[cfg(feature = "sensitivity")]
fn solve_dc_pf(net: &Network, req: &SolveRequest) -> Result<SolveResponse, String> {
    // Flow limits do not constrain a power flow, so a rating edit cannot enter
    // the model.
    reject_rating_deltas(&req.edits.rates, "dcpf")?;
    let mut dc = DcNetwork::from_network(net)?;
    let base = dc.base_mva;
    apply_demand_deltas(&mut dc, &req.edits.deltas)?;

    // Net per-unit injection per dense bus: generator setpoints minus (edited) load.
    // The slack absorbs the imbalance; its injection entry is recomputed, not echoed.
    let mut injection: Vec<f64> = dc.demand.iter().map(|d| -d).collect();
    for j in 0..dc.k {
        injection[dc.gen_bus[j]] += net.generators[dc.gen_ids[j] - 1].pg / base;
    }
    let sol = super::problem::dc_pf(&dc, &injection)?;

    Ok(SolveResponse {
        formulation: Problem::DcPf,
        status: SolveStatus::Feasible,
        objective: None,
        iterations: None,
        lmp: None,
        lmp_q: None,
        vm: None,
        va: Some(zip_bus(&dc.bus_ids, &dc.bus_uids, &sol.va)),
        w: None,
        injections: None,
        flows: Some(dc_branch_flows(
            &dc.branch_ids,
            &dc.branch_uids,
            &sol.f,
            &dc.fmax,
            base,
        )),
        dispatch: None,
        sensitivities: Vec::new(),
    })
}

#[cfg(feature = "sensitivity")]
fn solve_ac_pf(net: &Network, req: &SolveRequest) -> Result<SolveResponse, String> {
    let (acnet, sol) = ac_pf_solved(super::model::AcNetwork::from_network(net)?, req)?;
    ac_pf_assemble(&acnet, &sol, req)
}

/// Apply the request's demand edits to an owned [`AcNetwork`] and solve the AC power
/// flow, returning the perturbed model and its solution (retained for previews).
#[cfg(feature = "sensitivity")]
pub(crate) fn ac_pf_solved(
    mut acnet: super::model::AcNetwork,
    req: &SolveRequest,
) -> Result<(super::model::AcNetwork, super::problem::AcPfSolution), String> {
    reject_rating_deltas(&req.edits.rates, "acpf")?;
    apply_demand_deltas_ac(&mut acnet, &req.edits.deltas)?;
    let sol = super::problem::ac_pf(&super::formulation::AcPolar::new(), &acnet)?;
    Ok((acnet, sol))
}

/// Assemble the AC power flow [`SolveResponse`] (and sensitivity cells) from a solved
/// model. Shared by the one-shot path and the cached [`Study`] path.
#[cfg(feature = "sensitivity")]
pub(crate) fn ac_pf_assemble(
    acnet: &super::model::AcNetwork,
    sol: &super::problem::AcPfSolution,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    let base = acnet.base_mva;
    let sensitivities = run_cells(&super::sens::AcNewton::new(acnet, sol), &req.sensitivities)?;

    Ok(SolveResponse {
        formulation: Problem::AcPf,
        status: SolveStatus::Feasible,
        objective: None,
        iterations: Some(Iterations::Newton {
            count: sol.iterations,
            residual: sol.residual,
        }),
        lmp: None,
        lmp_q: None,
        vm: Some(zip_bus(&acnet.bus_ids, &acnet.bus_uids, &sol.vm)),
        va: Some(zip_bus(&acnet.bus_ids, &acnet.bus_uids, &sol.va)),
        w: None,
        injections: Some(zip_injections(
            &acnet.bus_ids,
            &acnet.bus_uids,
            &sol.p,
            &sol.q,
            base,
        )),
        flows: None,
        dispatch: None,
        sensitivities,
    })
}

#[cfg(feature = "conic")]
fn solve_socwr(net: &Network, req: &SolveRequest) -> Result<SolveResponse, String> {
    let (acnet, sol) = socwr_solved(super::model::AcNetwork::from_network(net)?, req)?;
    socwr_assemble(&acnet, &sol, req)
}

/// Apply the request's demand edits to an owned [`AcNetwork`] and solve the SOCWR
/// relaxation, returning the perturbed model and its solution (retained for previews).
/// Kept separate from [`socwr_assemble`] so a [`Study`](crate::study::Study) can retain
/// the solved model + solution and build a `ConicKkt` without re-solving.
#[cfg(feature = "conic")]
pub(crate) fn socwr_solved(
    mut acnet: super::model::AcNetwork,
    req: &SolveRequest,
) -> Result<(super::model::AcNetwork, super::problem::SocWrSolution), String> {
    apply_demand_deltas_ac(&mut acnet, &req.edits.deltas)?;
    apply_rating_deltas_ac(&mut acnet, &req.edits.rates)?;
    let sol = super::problem::socwr_opf(&acnet)?;
    Ok((acnet, sol))
}

/// Assemble the SOCWR [`SolveResponse`] (and sensitivity cells) from a solved model.
/// Shared by the one-shot path and the cached [`Study`] path.
#[cfg(feature = "conic")]
pub(crate) fn socwr_assemble(
    acnet: &super::model::AcNetwork,
    sol: &super::problem::SocWrSolution,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    use super::sens::ConicKkt;
    let base = acnet.base_mva;

    let sensitivities = {
        let sys = ConicKkt::new(acnet, sol).map_err(|e| e.to_string())?;
        run_cells(&sys, &req.sensitivities)?
    };

    Ok(SolveResponse {
        formulation: Problem::Socwr,
        status: SolveStatus::Optimal,
        objective: Some(sol.objective),
        iterations: Some(Iterations::Ipm(sol.iterations.clone())),
        lmp: Some(zip_scaled(
            &acnet.bus_ids,
            &acnet.bus_uids,
            &sol.lmp,
            1.0 / base,
        )),
        lmp_q: None,
        vm: None,
        va: None,
        w: Some(zip_bus(&acnet.bus_ids, &acnet.bus_uids, &sol.w)),
        injections: None,
        flows: Some(ac_branch_flows(
            &acnet.branch_ids,
            &acnet.branch_uids,
            &sol.pf,
            &sol.qf,
            &sol.pt,
            &sol.qt,
            &acnet.rate_a,
            base,
        )),
        dispatch: Some(zip_gen_pq(&acnet.gen_ids, &sol.pg, &sol.qg, base)),
        sensitivities,
    })
}

// ---------------------------------------------------------------------------
// Cached DC fast path (build the model once, solve per request)
// ---------------------------------------------------------------------------

/// Solve the DC OPF at `base + edits` from an already-built [`DcNetwork`]. The
/// constant topology (susceptance, limits, id maps, reference bus) is reused; only
/// the demand vector is perturbed. A server builds the model once per case and calls
/// this on every solve, so a demand drag never re-runs the normalize-and-reindex
/// that [`DcNetwork::from_network`] performs.
pub fn solve_prebuilt(dc: &DcNetwork, req: &SolveRequest) -> Result<SolveResponse, String> {
    solve_prebuilt_cancellable(dc, req, None)
}

/// As [`solve_prebuilt`], threading an optional cancel flag into the solve so a
/// timed-out or abandoned solve can be stopped at the next interior-point iteration.
pub fn solve_prebuilt_cancellable(
    base_dc: &DcNetwork,
    req: &SolveRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<SolveResponse, String> {
    // Clone the cached model so the perturbation never touches it; every field but
    // demand is constant for the case, so this is a flat Vec copy.
    dc_opf_response(base_dc.clone(), req, cancel)
}

// ---------------------------------------------------------------------------
// Sensitivity cells
// ---------------------------------------------------------------------------

/// Run each requested cell against the solved system and rescale to served units.
/// Takes `&dyn Differentiable` — the contract type — so every concrete system
/// (`DcKkt`, `AcNewton`, `ConicKkt`) coerces here; the `dyn` boundary is crossed once
/// per cell, never inside the linear algebra.
#[cfg(feature = "sensitivity")]
pub(crate) fn run_cells(
    sys: &dyn Differentiable,
    cells: &[SensRequest],
) -> Result<Vec<SensitivityMatrix>, String> {
    cells
        .iter()
        .map(|c| {
            let mut m = sensitivity(sys, c.operand, c.parameter, c.indices.as_deref(), c.mode)
                .map_err(|e| e.to_string())?;
            rescale_to_served(
                &mut m,
                sys.unit_scale(c.operand, c.parameter),
                c.operand,
                c.parameter,
            );
            Ok(m)
        })
        .collect()
}

/// Apply the per-unit -> served-unit rescale to a sensitivity matrix at the api edge:
/// multiply by the cell's `unit_scale` and stamp the served-unit label.
#[cfg(feature = "sensitivity")]
fn rescale_to_served(m: &mut SensitivityMatrix, scale: f64, op: Operand, par: Parameter) {
    if scale != 1.0 {
        for row in &mut m.values {
            for v in row {
                *v *= scale;
            }
        }
    }
    m.units = served_units_label(op, par);
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// What one formulation can do in this binary: whether it is built, the named
/// output blocks it populates, and (when the `sensitivity` feature is on) the
/// operands and parameters it supports. Any (operand, parameter) pair drawn from
/// the two lists is a valid sensitivity cell, so the UI takes the cross product.
#[derive(Clone, Debug, Serialize)]
pub struct ProblemCaps {
    pub formulation: Problem,
    /// Built in this binary (acopf is always `false`; it is not in this build).
    pub available: bool,
    /// Output blocks this formulation fills, e.g. `["lmp","va","flows","dispatch"]`.
    pub blocks: Vec<&'static str>,
    #[cfg(feature = "sensitivity")]
    pub operands: Vec<Operand>,
    #[cfg(feature = "sensitivity")]
    pub parameters: Vec<Parameter>,
}

/// The capability matrix as JSON, so the UI populates formulation/operand/parameter
/// menus and greys out unsupported combinations with no round-trip. The support set
/// is structural (a function of the formulation), so this takes no network. A
/// `#[cfg(test)]` guard probes each system on the bundled 3-bus case and asserts the
/// static lists match the engine, so the matrix cannot silently drift.
pub fn capabilities_json() -> String {
    serde_json::to_string(&formulation_caps()).unwrap_or_else(|e| e.to_string())
}

fn formulation_caps() -> Vec<ProblemCaps> {
    vec![
        ProblemCaps {
            formulation: Problem::DcPf,
            available: cfg!(feature = "sensitivity"),
            blocks: vec!["va", "flows"],
            #[cfg(feature = "sensitivity")]
            operands: vec![],
            #[cfg(feature = "sensitivity")]
            parameters: vec![],
        },
        ProblemCaps {
            formulation: Problem::DcOpf,
            available: true,
            blocks: vec!["lmp", "va", "flows", "dispatch"],
            #[cfg(feature = "sensitivity")]
            operands: vec![
                Operand::Price(Power::Active),
                Operand::Dispatch(Power::Active),
                Operand::Flow {
                    power: Power::Active,
                    end: End::From,
                },
                Operand::Voltage(VoltageKind::Angle),
            ],
            #[cfg(feature = "sensitivity")]
            parameters: vec![
                Parameter::Demand(Power::Active),
                Parameter::Cost(CostTerm::Quadratic),
                Parameter::Cost(CostTerm::Linear),
                Parameter::LineLimit,
                Parameter::SeriesAdmittance(GB::Susceptance),
                Parameter::Switching,
            ],
        },
        ProblemCaps {
            formulation: Problem::AcPf,
            available: cfg!(feature = "sensitivity"),
            blocks: vec!["vm", "va", "injections"],
            #[cfg(feature = "sensitivity")]
            operands: vec![
                Operand::Voltage(VoltageKind::Magnitude),
                Operand::Voltage(VoltageKind::Angle),
                Operand::Flow {
                    power: Power::Active,
                    end: End::From,
                },
                Operand::Flow {
                    power: Power::Active,
                    end: End::To,
                },
                Operand::Flow {
                    power: Power::Reactive,
                    end: End::From,
                },
                Operand::Flow {
                    power: Power::Reactive,
                    end: End::To,
                },
            ],
            #[cfg(feature = "sensitivity")]
            parameters: vec![
                Parameter::Demand(Power::Active),
                Parameter::Demand(Power::Reactive),
            ],
        },
        ProblemCaps {
            formulation: Problem::Socwr,
            available: cfg!(feature = "conic"),
            blocks: vec!["lmp", "w", "flows", "dispatch"],
            #[cfg(feature = "sensitivity")]
            operands: vec![
                Operand::Dispatch(Power::Active),
                Operand::Dispatch(Power::Reactive),
                Operand::Price(Power::Active),
                Operand::Price(Power::Reactive),
                Operand::Voltage(VoltageKind::Squared),
                Operand::Voltage(VoltageKind::ProductReal),
                Operand::Voltage(VoltageKind::ProductImag),
                Operand::Flow {
                    power: Power::Active,
                    end: End::From,
                },
                Operand::Flow {
                    power: Power::Active,
                    end: End::To,
                },
                Operand::Flow {
                    power: Power::Reactive,
                    end: End::From,
                },
                Operand::Flow {
                    power: Power::Reactive,
                    end: End::To,
                },
            ],
            #[cfg(feature = "sensitivity")]
            parameters: vec![
                Parameter::Demand(Power::Active),
                Parameter::Demand(Power::Reactive),
                Parameter::LineLimit,
                Parameter::VoltageBound(Bound::Min),
                Parameter::VoltageBound(Bound::Max),
                Parameter::GenBound {
                    power: Power::Active,
                    bound: Bound::Min,
                },
                Parameter::GenBound {
                    power: Power::Active,
                    bound: Bound::Max,
                },
                Parameter::GenBound {
                    power: Power::Reactive,
                    bound: Bound::Min,
                },
                Parameter::GenBound {
                    power: Power::Reactive,
                    bound: Bound::Max,
                },
                Parameter::Cost(CostTerm::Quadratic),
                Parameter::Cost(CostTerm::Linear),
                Parameter::SeriesAdmittance(GB::Conductance),
                Parameter::SeriesAdmittance(GB::Susceptance),
                Parameter::ShuntAdmittance(GB::Conductance),
                Parameter::ShuntAdmittance(GB::Susceptance),
            ],
        },
        // Full nonlinear AC OPF: not in this build. The entry is kept (with the same
        // output blocks it would fill) so the `acopf` tag stays in the matrix and the UI
        // can grey it out, but `available` is `false` and it offers no sensitivity cells.
        ProblemCaps {
            formulation: Problem::Acopf,
            available: false,
            blocks: vec!["lmp", "lmp_q", "vm", "va", "flows", "dispatch"],
            #[cfg(feature = "sensitivity")]
            operands: vec![],
            #[cfg(feature = "sensitivity")]
            parameters: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------
// Element-id joins and edit application
// ---------------------------------------------------------------------------

/// Original element id → dense model index, for any id-ordered axis (buses,
/// branches).
fn id_index_map(ids: &[usize]) -> HashMap<usize, usize> {
    ids.iter().enumerate().map(|(i, &id)| (id, i)).collect()
}

/// powerio row uid → dense model index, over a model's dense-aligned uid vector.
/// Built only when an edit set actually carries a uid key, so numeric-id clients
/// pay nothing for the uid path.
fn uid_index_map(uids: &[Option<String>]) -> HashMap<&str, usize> {
    uids.iter()
        .enumerate()
        .filter_map(|(i, uid)| uid.as_deref().map(|u| (u, i)))
        .collect()
}

/// Dense-index resolution maps for one edit axis: numeric ids always, uids only
/// when `keys` contains a uid (see [`uid_index_map`]).
struct KeyIndex<'a> {
    ids: HashMap<usize, usize>,
    uids: Option<HashMap<&'a str, usize>>,
}

impl<'a> KeyIndex<'a> {
    fn new(
        ids: &[usize],
        uids: &'a [Option<String>],
        keys: &HashMap<ElementKey, f64>,
    ) -> KeyIndex<'a> {
        let needs_uids = keys.keys().any(|k| matches!(k, ElementKey::Uid(_)));
        KeyIndex {
            ids: id_index_map(ids),
            uids: needs_uids.then(|| uid_index_map(uids)),
        }
    }

    /// Resolve one key to a dense index; `None` for an unknown element. A numeric
    /// id is cast through `usize::try_from`, not `as`, so an id that doesn't fit
    /// `usize` (reachable on the 32-bit wasm32 target) is rejected as unknown
    /// instead of silently truncating onto whatever element the wrapped value
    /// happens to name.
    fn get(&self, key: &ElementKey) -> Option<usize> {
        match key {
            ElementKey::Id(id) => {
                let id = usize::try_from(*id).ok()?;
                self.ids.get(&id).copied()
            }
            ElementKey::Uid(uid) => self.uids.as_ref()?.get(uid.as_str()).copied(),
        }
    }
}

/// `deltas` sorted by key. `HashMap`'s randomized hashing means iterating it
/// directly could surface a different validation error first on different runs of
/// the same invalid request; a deterministic order keeps `apply_demand_deltas`'s
/// error a function of the request alone.
fn sorted_deltas(deltas: &HashMap<ElementKey, f64>) -> Vec<(&ElementKey, f64)> {
    let mut entries: Vec<(&ElementKey, f64)> = deltas.iter().map(|(k, &mw)| (k, mw)).collect();
    entries.sort_unstable_by_key(|&(k, _)| k);
    entries
}

/// Validate one demand delta and resolve its bus to a dense index. Shared by the DC
/// and AC/SOCWR appliers so a positive bus key, a finite delta, a known bus, and a
/// delta that doesn't drive demand negative are enforced identically for both.
fn resolve_demand_delta(
    bus: &ElementKey,
    mw: f64,
    idx: &KeyIndex<'_>,
    base_mva: f64,
    current_demand_pu: impl Fn(usize) -> f64,
) -> Result<usize, String> {
    if matches!(bus, ElementKey::Id(id) if *id <= 0) {
        return Err("demand delta bus must be positive".into());
    }
    if !mw.is_finite() {
        return Err(format!("demand delta for bus {bus} must be finite"));
    }
    let i = idx
        .get(bus)
        .ok_or_else(|| format!("unknown demand delta bus {bus}"))?;
    if current_demand_pu(i) * base_mva + mw < -1e-9 {
        return Err(format!(
            "demand delta for bus {bus} would make demand negative"
        ));
    }
    Ok(i)
}

/// Establish the operating point: `demand += delta` (per unit) at each named bus.
fn apply_demand_deltas(
    dc: &mut DcNetwork,
    deltas: &HashMap<ElementKey, f64>,
) -> Result<(), String> {
    let base = dc.base_mva;
    let idx = KeyIndex::new(&dc.bus_ids, &dc.bus_uids, deltas);
    for (bus, mw) in sorted_deltas(deltas) {
        let i = resolve_demand_delta(bus, mw, &idx, base, |i| dc.demand[i])?;
        dc.demand[i] += mw / base;
    }
    Ok(())
}

/// AC analogue of [`apply_demand_deltas`]: active-power demand deltas onto `pd`.
#[cfg(feature = "sensitivity")]
fn apply_demand_deltas_ac(
    acnet: &mut super::model::AcNetwork,
    deltas: &HashMap<ElementKey, f64>,
) -> Result<(), String> {
    let base = acnet.base_mva;
    let idx = KeyIndex::new(&acnet.bus_ids, &acnet.bus_uids, deltas);
    for (bus, mw) in sorted_deltas(deltas) {
        let i = resolve_demand_delta(bus, mw, &idx, base, |i| acnet.pd[i])?;
        acnet.pd[i] += mw / base;
    }
    Ok(())
}

/// Validate one branch rating delta and resolve its branch to a dense index. Mirrors
/// [`resolve_demand_delta`]: a positive branch key, a finite delta, a known in-model
/// branch, and a delta that keeps the limit positive are enforced identically for
/// the DC and SOCWR appliers.
fn resolve_rating_delta(
    branch: &ElementKey,
    mw: f64,
    idx: &KeyIndex<'_>,
    base_mva: f64,
    current_limit_pu: impl Fn(usize) -> f64,
) -> Result<usize, String> {
    if matches!(branch, ElementKey::Id(id) if *id <= 0) {
        return Err("rating delta branch must be positive".into());
    }
    if !mw.is_finite() {
        return Err(format!("rating delta for branch {branch} must be finite"));
    }
    let i = idx
        .get(branch)
        .ok_or_else(|| format!("unknown rating delta branch {branch}"))?;
    if current_limit_pu(i) * base_mva + mw <= 1e-9 {
        return Err(format!(
            "rating delta for branch {branch} would make the line limit non-positive"
        ));
    }
    Ok(i)
}

/// Perturb the thermal limits: `fmax += delta` (per unit) at each named branch.
fn apply_rating_deltas(dc: &mut DcNetwork, rates: &HashMap<ElementKey, f64>) -> Result<(), String> {
    let base = dc.base_mva;
    let idx = KeyIndex::new(&dc.branch_ids, &dc.branch_uids, rates);
    for (branch, mw) in sorted_deltas(rates) {
        let i = resolve_rating_delta(branch, mw, &idx, base, |i| dc.fmax[i])?;
        dc.fmax[i] += mw / base;
    }
    Ok(())
}

/// SOCWR analogue of [`apply_rating_deltas`]: rating deltas onto the apparent-power
/// limit `rate_a`.
#[cfg(feature = "conic")]
fn apply_rating_deltas_ac(
    acnet: &mut super::model::AcNetwork,
    rates: &HashMap<ElementKey, f64>,
) -> Result<(), String> {
    let base = acnet.base_mva;
    let idx = KeyIndex::new(&acnet.branch_ids, &acnet.branch_uids, rates);
    for (branch, mw) in sorted_deltas(rates) {
        let i = resolve_rating_delta(branch, mw, &idx, base, |i| acnet.rate_a[i])?;
        acnet.rate_a[i] += mw / base;
    }
    Ok(())
}

/// The AC power flow has no flow limits, so a rating edit cannot enter the model;
/// reject it instead of silently solving without it.
#[cfg(feature = "sensitivity")]
fn reject_rating_deltas(rates: &HashMap<ElementKey, f64>, formulation: &str) -> Result<(), String> {
    if rates.is_empty() {
        return Ok(());
    }
    Err(format!(
        "branch rating edits are not supported by {formulation}"
    ))
}

/// The uid for dense index `i`, cloned off a model's dense-aligned uid vector.
fn uid_at(uids: &[Option<String>], i: usize) -> Option<String> {
    uids.get(i).cloned().flatten()
}

fn zip_bus(ids: &[usize], uids: &[Option<String>], vals: &[f64]) -> Vec<BusScalar> {
    ids.iter()
        .zip(vals)
        .enumerate()
        .map(|(i, (&bus, &value))| BusScalar {
            bus,
            uid: uid_at(uids, i),
            value,
        })
        .collect()
}

#[cfg(feature = "conic")]
fn zip_scaled(ids: &[usize], uids: &[Option<String>], vals: &[f64], scale: f64) -> Vec<BusScalar> {
    ids.iter()
        .zip(vals)
        .enumerate()
        .map(|(i, (&bus, &v))| BusScalar {
            bus,
            uid: uid_at(uids, i),
            value: v * scale,
        })
        .collect()
}

fn zip_gen_pg(gen_ids: &[usize], pg: &[f64], base: f64) -> Vec<GenDispatch> {
    gen_ids
        .iter()
        .zip(pg)
        .map(|(&gen, &p)| GenDispatch {
            gen,
            pg: p * base,
            qg: None,
        })
        .collect()
}

#[cfg(feature = "conic")]
fn zip_gen_pq(gen_ids: &[usize], pg: &[f64], qg: &[f64], base: f64) -> Vec<GenDispatch> {
    gen_ids
        .iter()
        .enumerate()
        .map(|(j, &gen)| GenDispatch {
            gen,
            pg: pg[j] * base,
            qg: Some(qg[j] * base),
        })
        .collect()
}

#[cfg(feature = "sensitivity")]
fn zip_injections(
    bus_ids: &[usize],
    bus_uids: &[Option<String>],
    p: &[f64],
    q: &[f64],
    base: f64,
) -> Vec<BusInjection> {
    bus_ids
        .iter()
        .enumerate()
        .map(|(i, &bus)| BusInjection {
            bus,
            uid: uid_at(bus_uids, i),
            p: p[i] * base,
            q: q[i] * base,
        })
        .collect()
}

/// DC branch flows: from-end active power (MW) and loading (|f|/limit). The reactive
/// and to-end legs are absent in DC.
fn dc_branch_flows(
    branch_ids: &[usize],
    branch_uids: &[Option<String>],
    f: &[f64],
    fmax: &[f64],
    base: f64,
) -> Vec<BranchFlow> {
    branch_ids
        .iter()
        .enumerate()
        .map(|(e, &branch)| {
            let loading = if fmax[e] > 0.0 {
                f[e].abs() / fmax[e]
            } else {
                0.0
            };
            BranchFlow {
                branch,
                uid: uid_at(branch_uids, e),
                pf: f[e] * base,
                loading,
                qf: None,
                pt: None,
                qt: None,
            }
        })
        .collect()
}

/// AC/conic branch flows: all four legs (MW/MVAr) and loading as the larger end's
/// apparent power over the rating (both per unit, dimensionless).
#[cfg(feature = "conic")]
#[allow(clippy::too_many_arguments)]
fn ac_branch_flows(
    branch_ids: &[usize],
    branch_uids: &[Option<String>],
    pf: &[f64],
    qf: &[f64],
    pt: &[f64],
    qt: &[f64],
    rate_a: &[f64],
    base: f64,
) -> Vec<BranchFlow> {
    branch_ids
        .iter()
        .enumerate()
        .map(|(e, &branch)| {
            let s_from = (pf[e] * pf[e] + qf[e] * qf[e]).sqrt();
            let s_to = (pt[e] * pt[e] + qt[e] * qt[e]).sqrt();
            let loading = if rate_a[e] > 0.0 {
                s_from.max(s_to) / rate_a[e]
            } else {
                0.0
            };
            BranchFlow {
                branch,
                uid: uid_at(branch_uids, e),
                pf: pf[e] * base,
                loading,
                qf: Some(qf[e] * base),
                pt: Some(pt[e] * base),
                qt: Some(qt[e] * base),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::model::CASE3;
    use super::*;
    use serde_json::Value;

    fn case3_json() -> String {
        powerio::parse_str(CASE3, "matpower")
            .expect("parse")
            .network
            .to_json()
            .expect("to_json")
    }

    fn case3_with_outages_json() -> String {
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse")
            .network;
        net.branches[0].in_service = false;
        net.generators[0].in_service = false;
        net.to_json().expect("to_json")
    }

    #[test]
    fn empty_request_defaults_to_dc_opf() {
        // `{}` and `""` both deserialize to a base-case DC OPF.
        for body in ["", "{}"] {
            let out = solve_json(&case3_json(), body).expect("solve");
            let v: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["formulation"], "dcopf");
            assert_eq!(v["status"], "optimal");
        }
    }

    #[test]
    fn dc_opf_payload_shapes() {
        let out = solve_json(&case3_json(), r#"{"formulation":"dcopf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["objective"].as_f64().unwrap() > 0.0);

        let lmp = v["lmp"].as_array().unwrap();
        assert_eq!(lmp.len(), 3);
        let buses: Vec<i64> = lmp.iter().map(|e| e["bus"].as_i64().unwrap()).collect();
        assert_eq!(buses, vec![1, 2, 3]);
        for e in lmp {
            assert!(e["value"].as_f64().unwrap() > 0.0);
        }

        assert_eq!(v["flows"].as_array().unwrap().len(), 3);
        let dispatch = v["dispatch"].as_array().unwrap();
        assert_eq!(dispatch.len(), 2);
        let total: f64 = dispatch.iter().map(|g| g["pg"].as_f64().unwrap()).sum();
        assert!((total - 90.0).abs() < 1e-2, "dispatch total {total}");

        // No sensitivity asked -> the array is omitted.
        assert!(v.get("sensitivities").is_none());

        // The interior-point trace is present for the solve plot.
        let iters = v["iterations"].as_array().unwrap();
        assert!(!iters.is_empty());
        for it in iters {
            assert!(it["inf_pr"].as_f64().unwrap().is_finite());
        }
    }

    #[test]
    fn deltas_shift_the_operating_point() {
        let base: Value =
            serde_json::from_str(&solve_json(&case3_json(), r#"{"formulation":"dcopf"}"#).unwrap())
                .unwrap();
        let bumped: Value = serde_json::from_str(
            &solve_json(
                &case3_json(),
                r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let lmp0 = base["lmp"][0]["value"].as_f64().unwrap();
        let lmp1 = bumped["lmp"][0]["value"].as_f64().unwrap();
        assert!(lmp1 > lmp0, "LMP should rise with demand: {lmp0} -> {lmp1}");
    }

    #[test]
    fn unknown_demand_delta_bus_errors() {
        let err = solve_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"999":1.0}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown demand delta bus 999"), "got: {err}");
    }

    #[test]
    fn demand_delta_cannot_make_demand_negative() {
        let err = solve_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":-1000.0}}}"#,
        )
        .unwrap_err();
        assert!(
            err.contains("demand delta for bus 2 would make demand negative"),
            "got: {err}"
        );
    }

    /// CASE3 with powerio row uids stamped by hand, in the `ensure_payload_uids`
    /// scheme (`buses:{row}` / `branches:{row}`). The engine crate does not depend
    /// on powerio-pkg; the wasm ingest path is what stamps real payloads.
    fn case3_with_uids_json() -> String {
        let mut net = powerio::parse_str(CASE3, "matpower")
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
    fn element_key_wire_forms() {
        // Value position: a JSON number and its decimal string both read as the
        // numeric id (object keys are always strings on the wire); anything else
        // reads as a uid.
        assert_eq!(
            serde_json::from_str::<ElementKey>("2").unwrap(),
            ElementKey::Id(2)
        );
        assert_eq!(
            serde_json::from_str::<ElementKey>("\"2\"").unwrap(),
            ElementKey::Id(2)
        );
        assert_eq!(
            serde_json::from_str::<ElementKey>("\"branches:1\"").unwrap(),
            ElementKey::Uid("branches:1".into())
        );
        // An id serializes back as a number, a uid as a string.
        assert_eq!(serde_json::to_string(&ElementKey::Id(2)).unwrap(), "2");
        assert_eq!(
            serde_json::to_string(&ElementKey::Uid("buses:0".into())).unwrap(),
            "\"buses:0\""
        );
    }

    #[test]
    fn uid_keyed_edits_match_id_keyed_edits() {
        // Bus id 2 is row 1 (`buses:1`); branch 3 is row 2 (`branches:2`). The same
        // edit through either key must build the same model, so the responses are
        // identical.
        let net = case3_with_uids_json();
        let by_id: Value = serde_json::from_str(
            &solve_json(
                &net,
                r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0},"rates":{"3":-25.0}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let by_uid: Value = serde_json::from_str(
            &solve_json(
                &net,
                r#"{"formulation":"dcopf","edits":{"deltas":{"buses:1":50.0},"rates":{"branches:2":-25.0}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(by_id["objective"], by_uid["objective"]);
        assert_eq!(by_id["lmp"], by_uid["lmp"]);
        assert_eq!(by_id["flows"], by_uid["flows"]);
    }

    #[test]
    fn unknown_uid_key_errors() {
        let err = solve_json(
            &case3_with_uids_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"buses:99":1.0}}}"#,
        )
        .unwrap_err();
        assert!(
            err.contains(r#"unknown demand delta bus "buses:99""#),
            "got: {err}"
        );
        // A network that was never stamped resolves no uid key at all.
        let err = solve_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"buses:1":1.0}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown demand delta bus"), "got: {err}");
    }

    #[test]
    fn response_scalars_echo_uids() {
        let out = solve_json(&case3_with_uids_json(), r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["lmp"][1]["uid"], "buses:1");
        assert_eq!(v["va"][0]["uid"], "buses:0");
        assert_eq!(v["flows"][0]["uid"], "branches:0");
        // A network without uids omits the field entirely.
        let out = solve_json(&case3_json(), r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["lmp"][1].get("uid").is_none());
        assert!(v["flows"][0].get("uid").is_none());
    }

    #[test]
    fn payload_ids_survive_out_of_service_elements() {
        let out =
            solve_json(&case3_with_outages_json(), r#"{"formulation":"dcopf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        let branches: Vec<i64> = v["flows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["branch"].as_i64().unwrap())
            .collect();
        let gens: Vec<i64> = v["dispatch"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["gen"].as_i64().unwrap())
            .collect();
        assert_eq!(branches, vec![2, 3]);
        assert_eq!(gens, vec![2]);
    }

    #[test]
    fn shed_option_controls_infeasibility() {
        // case3 with generation cut below the 0.9 pu load: unservable without shedding.
        let net = powerio::parse_str(CASE3, "matpower")
            .expect("parse")
            .network;
        let mut dc = DcNetwork::from_network(&net).expect("model");
        dc.gmax = vec![0.4, 0.4]; // 0.8 pu capacity < 0.9 pu load

        let off = solve_prebuilt(&dc, &SolveRequest::default());
        assert!(
            off.is_err(),
            "expected infeasible without shedding, got {off:?}"
        );

        let on = solve_prebuilt(
            &dc,
            &SolveRequest {
                options: SolveOptions {
                    shed: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("shed-on solve");
        let gen_mw: f64 = on.dispatch.unwrap().iter().map(|d| d.pg).sum();
        assert!(
            gen_mw < 90.0 - 1.0,
            "shed-on should shed (dispatched {gen_mw})"
        );
    }

    #[test]
    fn capabilities_lists_formulations() {
        let v: Value = serde_json::from_str(&capabilities_json()).unwrap();
        let arr = v.as_array().unwrap();
        let tags: Vec<&str> = arr
            .iter()
            .map(|f| f["formulation"].as_str().unwrap())
            .collect();
        assert_eq!(tags, vec!["dcpf", "dcopf", "acpf", "socwr", "acopf"]);
        // DC OPF is always built; acopf is not in this build, so it reports unavailable
        // (the tag stays in the matrix for a stable wire contract).
        let dc_opf = arr.iter().find(|f| f["formulation"] == "dcopf").unwrap();
        assert_eq!(dc_opf["available"], true);
        let acopf = arr.iter().find(|f| f["formulation"] == "acopf").unwrap();
        assert_eq!(acopf["available"], false);
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn dc_opf_sensitivity_cell() {
        // sens_bus 2 is dense index 1 in case3 (bus ids 1, 2, 3).
        let req = r#"{"formulation":"dcopf","sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}]}"#;
        let out = solve_json(&case3_json(), req).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        let sens = v["sensitivities"].as_array().unwrap();
        assert_eq!(sens.len(), 1);
        let m = &sens[0];
        assert_eq!(m["units"], "($/MWh)/MW");
        assert_eq!(m["cols"].as_array().unwrap()[0]["element"]["Bus"], 2);
        let rows = m["values"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        for r in rows {
            assert!(r.as_array().unwrap()[0].as_f64().unwrap() > 0.0);
        }
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn unsupported_cell_errors() {
        // DC has no W-space squared voltage.
        let req = r#"{"formulation":"dcopf","sensitivities":[{"operand":{"Voltage":"Squared"},"parameter":{"Demand":"Active"}}]}"#;
        let err = solve_json(&case3_json(), req).unwrap_err();
        assert!(err.contains("does not support"), "got: {err}");
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn ac_pf_reports_voltages_and_injections() {
        let out = solve_json(&case3_json(), r#"{"formulation":"acpf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "acpf");
        assert_eq!(v["vm"].as_array().unwrap().len(), 3);
        assert_eq!(v["va"].as_array().unwrap().len(), 3);
        assert_eq!(v["injections"].as_array().unwrap().len(), 3);
        assert!(v["lmp"].is_null());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn dc_pf_reports_angles_and_flows() {
        let out = solve_json(&case3_json(), r#"{"formulation":"dcpf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "dcpf");
        assert_eq!(v["va"].as_array().unwrap().len(), 3);
        assert_eq!(v["flows"].as_array().unwrap().len(), 3);
        assert!(v["lmp"].is_null());
        assert!(v["dispatch"].is_null());
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_reports_w_and_reactive_capable_sensitivity() {
        let req = r#"{"formulation":"socwr","sensitivities":[{"operand":{"Price":"Reactive"},"parameter":{"Demand":"Active"}}]}"#;
        let out = solve_json(&case3_json(), req).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "socwr");
        assert_eq!(v["w"].as_array().unwrap().len(), 3);
        let m = &v["sensitivities"].as_array().unwrap()[0];
        assert_eq!(m["units"], "($/MVArh)/MW");
        for row in m["values"].as_array().unwrap() {
            for x in row.as_array().unwrap() {
                assert!(x.as_f64().unwrap().is_finite());
            }
        }
    }

    #[test]
    fn acopf_is_not_available_in_this_build() {
        // The full nonlinear AC OPF is not built on this branch; requesting it errors
        // cleanly rather than degrading silently.
        let err = solve_json(&case3_json(), r#"{"formulation":"acopf"}"#).unwrap_err();
        assert!(err.contains("not available in this build"), "got: {err}");
    }

    /// Guard against the static capability matrix drifting from the engine: build each
    /// available system on case3 and assert every operand/parameter the matrix lists is
    /// one the engine actually supports (`operand_len`/`parameter_len` are `Some`). Covers
    /// DC OPF and AC PF always, and SOCWR behind the `conic` feature, so a UI menu driven
    /// by `capabilities_json` can never offer a cell that errors at solve time.
    /// (DESIGN §8 open item: the guard used to probe only `dcopf`.)
    #[cfg(feature = "sensitivity")]
    #[test]
    fn capabilities_match_engine() {
        use super::super::formulation::AcPolar;
        use super::super::model::{AcNetwork, DcNetwork};
        use super::super::problem::{ac_pf, dc_opf};
        use super::super::sens::{AcNewton, DcKkt, Differentiable};

        let net = powerio::parse_str(CASE3, "matpower").unwrap().network;
        let caps = formulation_caps();

        // Every operand/parameter the matrix lists for `f` must be engine-supported.
        let check = |f: Problem, sys: &dyn Differentiable| {
            let c = caps.iter().find(|c| c.formulation == f).unwrap();
            assert!(
                c.available,
                "{f:?} probed but the matrix lists it unavailable"
            );
            for o in &c.operands {
                assert!(
                    sys.operand_len(*o).is_some(),
                    "{f:?}: listed operand {o:?} unsupported by the engine"
                );
            }
            for p in &c.parameters {
                assert!(
                    sys.parameter_len(*p).is_some(),
                    "{f:?}: listed parameter {p:?} unsupported by the engine"
                );
            }
        };

        // DC OPF (always available under `sensitivity`).
        let dc = DcNetwork::from_network(&net).unwrap();
        let dc_sol = dc_opf(&dc).unwrap();
        check(Problem::DcOpf, &DcKkt::new(&dc, &dc_sol));

        // AC power flow (Newton system).
        let ac = AcNetwork::from_network(&net).unwrap();
        let ac_sol = ac_pf(&AcPolar::new(), &ac).unwrap();
        check(Problem::AcPf, &AcNewton::new(&ac, &ac_sol));

        // SOCWR conic relaxation.
        #[cfg(feature = "conic")]
        {
            let soc = super::super::problem::socwr_opf(&ac).unwrap();
            let sys = super::super::sens::ConicKkt::new(&ac, &soc).unwrap();
            check(Problem::Socwr, &sys);
        }
    }
}
