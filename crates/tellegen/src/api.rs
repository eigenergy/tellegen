//! The browser- and server-facing entry point: one driver over every formulation.
//!
//! Read a PowerIO module or typed problem instance, apply operating point edits, solve the requested
//! formulation, attach any requested sensitivity cells, and serve a
//! formulation-agnostic response. The frontend picks three things in one request:
//! the **problem** it solves (`dcpf`/`dcopf`/`acpf`/`socwr`), the **operand** it
//! differentiates, and the **parameter** it differentiates with respect to. The same
//! physical vocabulary the [`sensitivity`] driver uses ([`Operand`]/[`Parameter`])
//! crosses the JSON edge unchanged.
//!
//! Keeping the JSON layer here (not behind `#[wasm_bindgen]`) makes it testable
//! natively; the wasm crate wraps [`solve_module_json`] and [`capabilities_json`].

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(feature = "conic")]
use powerio::AcOpfInstance;
#[cfg(feature = "sensitivity")]
use powerio::AcPfInstance;
use powerio::BalancedNetwork;
use powerio::DcOpfInstance;
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
/// was built; a uid the network does not carry is an unknown-element error.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElementKey {
    /// Original numeric id: bus id or 1-based branch position.
    Id(i64),
    /// powerio row uid, e.g. `"branches:1"`.
    Uid(String),
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for ElementKey {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ElementKey".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "oneOf": [{ "type": "integer" }, { "type": "string" }] })
    }
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Edits {
    /// Active-power demand delta in MW per bus key.
    #[serde(default)]
    pub deltas: HashMap<ElementKey, f64>,
    /// Thermal rating delta in MW per branch key. Supported by the DC OPF
    /// and SOCWR paths; the AC power flow has no flow limits and rejects them.
    #[serde(default)]
    pub rates: HashMap<ElementKey, f64>,
}

/// One requested sensitivity cell: an [`Operand`] differentiated with respect to a
/// [`Parameter`], over an optional parameter-index subset, in an optional direction.
/// The operand/parameter are the contract's serde-tagged enums verbatim
/// (`{"Price":"Active"}` / `{"Demand":"Active"}`).
#[cfg(feature = "sensitivity")]
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// The quantity a [`ConstraintTerm`] weights, as the snake_case JSON tags
/// `"branch_flow"`, `"bus_injection"`, and `"generator"`. Every quantity is
/// active power in MW.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConstraintTermKind {
    /// A branch's from-end active flow, positive from its `from` bus toward its
    /// `to` bus. Keyed like a rating edit: the 1-based branch position or the
    /// branch uid.
    BranchFlow,
    /// A bus's net active injection: generation minus demand and shunt
    /// conductance withdrawal, which equals the flow leaving the bus on its
    /// branches. Keyed like a demand edit: the bus id or the bus uid.
    BusInjection,
    /// A generator's active output. Keyed by the 1-based generator position or
    /// the generator uid.
    Generator,
}

/// One weighted term of a [`LinearConstraint`]: `coefficient` times the MW
/// quantity `kind` names at `element`. Terms naming the same quantity add.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConstraintTerm {
    pub kind: ConstraintTermKind,
    pub element: ElementKey,
    pub coefficient: f64,
}

/// A caller-supplied linear constraint on the DC OPF, such as an interface or
/// transfer limit: `lower <= sum(coefficient * quantity) <= upper`, in MW. Either
/// limit may be omitted, not both; equal limits state an equality. The row is
/// enforced alongside the network's own limits, and the response reports its
/// value and shadow price in [`SolveResponse::constraints`].
///
/// ```json
/// { "id": "north-south", "upper": 400.0,
///   "terms": [ { "kind": "branch_flow", "element": 3, "coefficient": 1.0 },
///              { "kind": "branch_flow", "element": 7, "coefficient": -1.0 } ] }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LinearConstraint {
    /// The caller's name for the row, unique within a request and echoed in the
    /// response.
    pub id: String,
    pub terms: Vec<ConstraintTerm>,
    /// Lower limit in MW.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,
    /// Upper limit in MW.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,
}

/// The one solve request: a formulation, an operating-point edit set, zero or more
/// caller-supplied linear constraints, and zero or more sensitivity cells. A bare
/// `{"formulation":"acpf"}` (or even `{}`, which defaults to DC OPF) is valid.
///
/// ```json
/// {
///   "formulation": "dcopf",
///   "edits": { "deltas": { "2": 50.0 }, "rates": { "3": -25.0 } },
///   "sensitivities": [
///     { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"}, "indices": [1] }
///   ]
/// }
/// ```
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SolveRequest {
    #[serde(default)]
    pub formulation: Problem,
    #[serde(default)]
    pub edits: Edits,
    /// Caller-supplied linear constraints, enforced by the DC OPF only. Every
    /// other formulation refuses a request that carries any. Sensitivity cells
    /// differentiate the program with them, and `ConstraintLimit` differentiates
    /// with respect to their limits.
    #[serde(default)]
    pub constraints: Vec<LinearConstraint>,
    /// Zero or more sensitivity cells, computed against the solved system in request
    /// order. Ignored by a build without the `sensitivity` feature.
    #[cfg(feature = "sensitivity")]
    #[serde(default)]
    pub sensitivities: Vec<SensRequest>,
}

/// Validate edit keys against the caller's canonical PowerIO tables before any
/// analysis lowering occurs. The solver view may contain star buses and winding
/// branches synthesized for a three-winding transformer; those rows are valid
/// model axes, but they are not source elements and therefore are never edit
/// targets on a public API.
pub(crate) fn validate_canonical_edits(net: &BalancedNetwork, edits: &Edits) -> Result<(), String> {
    validate_canonical_identity(net)?;
    for (bus, mw) in sorted_deltas(&edits.deltas) {
        if !mw.is_finite() {
            return Err(format!("demand delta for bus {bus} must be finite"));
        }
        let target = match bus {
            ElementKey::Id(id) if *id <= 0 => {
                return Err("demand delta bus must be positive".into());
            }
            ElementKey::Id(id) => usize::try_from(*id)
                .ok()
                .and_then(|id| net.buses().iter().find(|bus| bus.id.0 == id)),
            ElementKey::Uid(uid) => net
                .buses()
                .iter()
                .find(|bus| bus.uid.as_deref() == Some(uid)),
        };
        let Some(target) = target else {
            return Err(format!("unknown demand delta bus {bus}"));
        };
        if target.kind == powerio::BusType::Isolated {
            return Err(format!("demand delta bus {bus} is not editable"));
        }
    }
    for (branch, mw) in sorted_deltas(&edits.rates) {
        if !mw.is_finite() {
            return Err(format!("rating delta for branch {branch} must be finite"));
        }
        let target = match branch {
            ElementKey::Id(id) if *id <= 0 => {
                return Err("rating delta branch must be positive".into());
            }
            ElementKey::Id(id) => usize::try_from(*id)
                .ok()
                .and_then(|id| id.checked_sub(1))
                .and_then(|row| net.branches().get(row)),
            ElementKey::Uid(uid) => net
                .branches()
                .iter()
                .find(|branch| branch.uid.as_deref() == Some(uid)),
        };
        let Some(target) = target else {
            return Err(format!("unknown rating delta branch {branch}"));
        };
        let endpoint_is_editable = |id| {
            net.buses()
                .iter()
                .any(|bus| bus.id == id && bus.kind != powerio::BusType::Isolated)
        };
        if !target.in_service
            || target.from == target.to
            || target.r * target.r + target.x * target.x == 0.0
            || !endpoint_is_editable(target.from)
            || !endpoint_is_editable(target.to)
        {
            return Err(format!("rating delta branch {branch} is not editable"));
        }
    }
    Ok(())
}

/// Require canonical row identity to be unambiguous on each editable axis.
///
/// A bus and a branch may intentionally share a uid because their edit axes are
/// distinct. Two buses (or two branches) may not: key lookup and module
/// persistence would otherwise disagree about which source row the uid names.
/// Missing uids remain valid for callers that use numeric keys.
pub fn validate_canonical_identity(net: &BalancedNetwork) -> Result<(), String> {
    super::model::validate_canonical_identity(net)
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// A solve outcome that succeeded. A failed solve is the `Err` arm of a solve entry.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BusScalar {
    pub bus: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub value: f64,
}

/// A nodal net injection (MW / MVAr), keyed by original bus id (plus the row uid
/// when carried, as in [`BusScalar`]). Injections describe the net nodal power balance.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BusInjection {
    pub bus: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub p: f64,
    pub q: f64,
}

/// A branch-indexed voltage product, in squared per unit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BranchScalar {
    pub branch: usize,
    pub value: f64,
}

/// Branch flows, keyed by original branch id (plus the row uid when carried, as in
/// [`BusScalar`]). `pf` (from-end active, MW) and `loading` (|S|/limit,
/// dimensionless) are present on every formulation that has flows; the reactive
/// and to-end legs are `None` on the DC paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GenDispatch {
    pub gen: usize,
    /// Original bus identity, allowing exact aggregation of co-located dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<usize>,
    pub pg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qg: Option<f64>,
}

/// The solved state of one [`LinearConstraint`], in request order. `value`,
/// `lower`, and `upper` are MW; `shadow_price` is in objective units per MW, the
/// same units as `lmp`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ConstraintResult {
    pub id: String,
    /// `sum(coefficient * quantity)` at the solution.
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,
    /// The objective decrease per MW the binding limit is raised:
    /// `-d objective / d limit`. Positive when the upper limit binds, negative
    /// when the lower limit binds, zero when neither does. A bus's price then
    /// differs from the reference bus's by `-shadow_price` times the change in
    /// `value` per MW injected at the bus and withdrawn at the reference.
    /// Absent when the declared objective gives prices no economic meaning, as
    /// for `lmp`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_price: Option<f64>,
    /// `value` sits at a limit, within solver tolerance. Always true for an
    /// equality.
    pub binding: bool,
}

/// The formulation-agnostic solve result. A superset: every block is optional, and
/// each formulation fills what it produces. Powers are MW/MVAr, nodal values are
/// in objective units per selected power unit, angles radians, `vm` per unit, and
/// `w = |V|^2` per unit squared. Element ids
/// are the original bus/branch/generator ids, so the frontend joins straight onto
/// its case.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SolveResponse {
    /// The formulation that produced this, echoed for the client.
    pub formulation: Problem,
    pub status: SolveStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iterations: Option<Iterations>,
    /// Active nodal price when the solved OPF declares a network generator
    /// cost objective. Feasibility solves leave this absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmp: Option<Vec<BusScalar>>,
    /// Reactive nodal price. Always `None` in this build; the field is retained for
    /// wire-format stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmp_q: Option<Vec<BusScalar>>,
    /// Voltage magnitude, per unit (ACPF or square root of SOCWR squared magnitude).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm: Option<Vec<BusScalar>>,
    /// Voltage angle, radians (every path except socwr, which is W-space).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub va: Option<Vec<BusScalar>>,
    /// Squared voltage magnitude `w = |V|^2`, per unit squared (socwr). The conic
    /// relaxation does not guarantee a globally consistent set of voltage angles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<Vec<BusScalar>>,
    /// Real and imaginary oriented branch voltage products (SOCWR).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wr: Option<Vec<BranchScalar>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wi: Option<Vec<BranchScalar>>,
    /// Nodal injections (acpf), MW/MVAr.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injections: Option<Vec<BusInjection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flows: Option<Vec<BranchFlow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<Vec<GenDispatch>>,
    /// The request's linear constraints, in request order (dcopf). Absent when
    /// the request stated none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<ConstraintResult>>,
    /// One self-describing matrix per requested cell, in request order. Each carries
    /// its own row/column element ids and the served-unit label.
    #[cfg(feature = "sensitivity")]
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sensitivities: Vec<SensitivityMatrix>,
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// Parse a network for crate tests, solve the requested formulation at `base + edits`,
/// attach every requested sensitivity cell, and return the [`SolveResponse`] as JSON.
#[cfg(test)]
pub(crate) fn solve_test_network_json(
    network_json: &str,
    request_json: &str,
) -> Result<String, String> {
    let net: BalancedNetwork = serde_json::from_str(network_json).map_err(|e| e.to_string())?;
    let req: SolveRequest = if request_json.trim().is_empty() {
        SolveRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|e| format!("bad request JSON: {e}"))?
    };
    let resp = solve_network(&net, &req)?;
    serde_json::to_string(&resp).map_err(|e| e.to_string())
}

/// Solve a PowerIO stored module. A balanced network is promoted to the
/// default problem instance for the requested formulation; a stored DC or AC
/// OPF instance keeps its declared objective and constraint selections.
pub fn solve_module_json(module_json: &str, request_json: &str) -> Result<String, String> {
    let module = crate::ir::deserialize_module(module_json)?;
    let req: SolveRequest = if request_json.trim().is_empty() {
        SolveRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|error| format!("bad request JSON: {error}"))?
    };
    let response = match module.into_value() {
        powerio::PioValue::BalancedNetwork(network) => match req.formulation {
            Problem::DcOpf => {
                let instance = DcOpfInstance::from_network(network).map_err(|e| e.to_string())?;
                solve_instance(&instance, &req)?
            }
            #[cfg(feature = "sensitivity")]
            Problem::AcPf => {
                let instance = AcPfInstance::from_network(network).map_err(|e| e.to_string())?;
                solve_ac_pf_instance(&instance, &req)?
            }
            #[cfg(feature = "conic")]
            Problem::Socwr => {
                let instance = AcOpfInstance::from_network(network).map_err(|e| e.to_string())?;
                solve_ac_instance(&instance, &req)?
            }
            _ => solve_network(&network, &req)?,
        },
        powerio::PioValue::DcOpfInstance(instance) => solve_instance(&instance, &req)?,
        #[cfg(feature = "sensitivity")]
        powerio::PioValue::AcPfInstance(instance) => solve_ac_pf_instance(&instance, &req)?,
        #[cfg(feature = "conic")]
        powerio::PioValue::AcOpfInstance(instance) => solve_ac_instance(&instance, &req)?,
        other => {
            return Err(format!(
                "PowerIO module holds {}, which this solve entry does not support",
                other.type_name()
            ));
        }
    };
    serde_json::to_string(&response).map_err(|error| error.to_string())
}

/// Solve an already-parsed [`BalancedNetwork`] under `req`. Dispatches on the formulation to
/// the matching solver, then runs each requested sensitivity against the matching
/// differentiable system. Problems this build does not include return a clean
/// `Err` rather than degrading silently.
pub(crate) fn solve_network(
    net: &BalancedNetwork,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    validate_canonical_edits(net, &req.edits)?;
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

/// Solve a typed PowerIO DC OPF instance. This is the public numerical entry
/// for callers that already own the problem declaration; the solver workspace
/// and its dense arrays remain private to Tellegen.
pub fn solve_instance(
    instance: &DcOpfInstance,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    solve_instance_cancellable(instance, req, None)
}

/// As [`solve_instance`], with cancellation polled by the interior point solve.
pub fn solve_instance_cancellable(
    instance: &DcOpfInstance,
    req: &SolveRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<SolveResponse, String> {
    if req.formulation != Problem::DcOpf {
        return Err(format!(
            "a dc_opf_instance cannot be solved as {:?}",
            req.formulation
        ));
    }
    validate_canonical_edits(instance.network(), &req.edits)?;
    dc_opf_response(DcNetwork::from_instance(instance)?, req, cancel)
}

/// Solve a typed PowerIO AC power flow instance. The instance's PQ, PV, and
/// reference specifications define the Newton system; they are not inferred
/// again from the network tables.
#[cfg(feature = "sensitivity")]
pub fn solve_ac_pf_instance(
    instance: &AcPfInstance,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    if req.formulation != Problem::AcPf {
        return Err(format!(
            "an ac_pf_instance cannot be solved as {:?}",
            req.formulation
        ));
    }
    validate_canonical_edits(instance.network(), &req.edits)?;
    let (model, solution) =
        ac_pf_solved(super::model::AcNetwork::from_pf_instance(instance)?, req)?;
    ac_pf_assemble(&model, &solution, req)
}

/// Solve a typed PowerIO AC OPF instance with the SOCWR relaxation. The
/// private conic workspace retains the instance's objective and active
/// constraint selections.
#[cfg(feature = "conic")]
pub fn solve_ac_instance(
    instance: &AcOpfInstance,
    req: &SolveRequest,
) -> Result<SolveResponse, String> {
    if req.formulation != Problem::Socwr {
        return Err(format!(
            "an ac_opf_instance cannot be solved as {:?}",
            req.formulation
        ));
    }
    validate_canonical_edits(instance.network(), &req.edits)?;
    let (model, solution) = socwr_solved(super::model::AcNetwork::from_instance(instance)?, req)?;
    socwr_assemble(&model, &solution, req)
}

fn solve_dc_opf(net: &BalancedNetwork, req: &SolveRequest) -> Result<SolveResponse, String> {
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
    dc.allow_shed = false;
    apply_demand_deltas(&mut dc, &req.edits.deltas)?;
    apply_rating_deltas(&mut dc, &req.edits.rates)?;
    apply_linear_constraints(&mut dc, &req.constraints)?;
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

    #[cfg(feature = "sensitivity")]
    if dc.objective != powerio_matrix::PreparedObjective::NetworkGeneratorCost
        && req
            .sensitivities
            .iter()
            .any(|cell| matches!(cell.operand, Operand::Price(_)))
    {
        return Err("price sensitivity requires a network_generator_cost objective".to_string());
    }

    #[cfg(feature = "sensitivity")]
    let sensitivities = run_cells(&super::sens::DcKkt::new(dc, sol), &req.sensitivities)?;

    let lmp_is_economic = dc.objective == powerio_matrix::PreparedObjective::NetworkGeneratorCost;
    let lmp = lmp_is_economic.then(|| {
        let values = sol.nodal_marginal_values(base);
        zip_bus(&dc.bus_ids, &dc.bus_uids, &values)
    });

    Ok(SolveResponse {
        formulation: Problem::DcOpf,
        status: SolveStatus::Optimal,
        objective: Some(sol.objective),
        iterations: Some(Iterations::Ipm(sol.iterations.clone())),
        lmp,
        lmp_q: None,
        vm: None,
        va: Some(zip_bus(&dc.bus_ids, &dc.bus_uids, &sol.va)),
        w: None,
        wr: None,
        wi: None,
        injections: None,
        flows: Some(dc_branch_flows(
            &dc.branch_ids,
            &dc.branch_uids,
            &sol.f,
            &dc.fmax,
            base,
        )),
        dispatch: Some(zip_gen_pg(dc, &sol.pg, base)),
        constraints: (!dc.linear_rows.is_empty())
            .then(|| linear_constraint_results(dc, sol, lmp_is_economic)),
        #[cfg(feature = "sensitivity")]
        sensitivities,
    })
}

/// Solve the DC OPF for an owned private workspace and assemble the response.
fn dc_opf_response(
    dc: DcNetwork,
    req: &SolveRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<SolveResponse, String> {
    let (dc, sol) = dc_opf_solved(dc, req, cancel)?;
    dc_opf_assemble(&dc, &sol, req)
}

#[cfg(feature = "sensitivity")]
fn solve_dc_pf(net: &BalancedNetwork, req: &SolveRequest) -> Result<SolveResponse, String> {
    // Flow limits do not constrain a power flow, so a rating edit cannot enter
    // the model, and neither can a linear constraint.
    reject_rating_deltas(&req.edits.rates, "dcpf")?;
    reject_linear_constraints(&req.constraints, "dcpf")?;
    let mut dc = DcNetwork::from_network(net)?;
    let base = dc.base_mva;
    apply_demand_deltas(&mut dc, &req.edits.deltas)?;

    // Net per-unit injection per dense bus: generator setpoints minus (edited) load.
    // The slack absorbs the imbalance; its injection entry is recomputed, not echoed.
    let mut injection: Vec<f64> = dc.demand.iter().map(|d| -d).collect();
    let input_power_base = if net.is_normalized() { 1.0 } else { base };
    for j in 0..dc.k {
        let source_row = dc.gen_source_rows[j]
            .ok_or_else(|| format!("generator column {j} has no source row"))?;
        injection[dc.gen_bus[j]] += net.generators()[source_row].pg / input_power_base;
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
        wr: None,
        wi: None,
        injections: None,
        flows: Some(dc_branch_flows(
            &dc.branch_ids,
            &dc.branch_uids,
            &sol.f,
            &dc.fmax,
            base,
        )),
        dispatch: None,
        constraints: None,
        sensitivities: Vec::new(),
    })
}

#[cfg(feature = "sensitivity")]
fn solve_ac_pf(net: &BalancedNetwork, req: &SolveRequest) -> Result<SolveResponse, String> {
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
    if acnet.has_remote_voltage_control {
        return Err("acpf does not yet support a generator regulating a remote bus".into());
    }
    reject_rating_deltas(&req.edits.rates, "acpf")?;
    reject_linear_constraints(&req.constraints, "acpf")?;
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
    let [pf, qf, pt, qt] = crate::emit::ac_terminal_flows(acnet, sol);
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
        wr: None,
        wi: None,
        injections: Some(zip_injections(
            &acnet.bus_ids,
            &acnet.bus_uids,
            &sol.p,
            &sol.q,
            base,
        )),
        flows: Some(ac_branch_flows(
            &acnet.branch_ids,
            &acnet.branch_uids,
            &pf,
            &qf,
            &pt,
            &qt,
            &acnet.rate_a,
            base,
        )),
        dispatch: None,
        constraints: None,
        sensitivities,
    })
}

#[cfg(feature = "conic")]
fn solve_socwr(net: &BalancedNetwork, req: &SolveRequest) -> Result<SolveResponse, String> {
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
    reject_linear_constraints(&req.constraints, "socwr")?;
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

    if acnet.objective != powerio_matrix::PreparedObjective::NetworkGeneratorCost
        && req
            .sensitivities
            .iter()
            .any(|cell| matches!(cell.operand, Operand::Price(_)))
    {
        return Err("price sensitivity requires a network_generator_cost objective".to_string());
    }

    let sensitivities = {
        let sys = ConicKkt::new(acnet, sol).map_err(|e| e.to_string())?;
        run_cells(&sys, &req.sensitivities)?
    };

    Ok(SolveResponse {
        formulation: Problem::Socwr,
        status: SolveStatus::Optimal,
        objective: Some(sol.objective),
        iterations: Some(Iterations::Ipm(sol.iterations.clone())),
        lmp: (acnet.objective == powerio_matrix::PreparedObjective::NetworkGeneratorCost)
            .then(|| zip_scaled(&acnet.bus_ids, &acnet.bus_uids, &sol.lmp, 1.0 / base)),
        lmp_q: (acnet.objective == powerio_matrix::PreparedObjective::NetworkGeneratorCost)
            .then(|| zip_scaled(&acnet.bus_ids, &acnet.bus_uids, &sol.lmp_q, 1.0 / base)),
        vm: Some(zip_bus(
            &acnet.bus_ids,
            &acnet.bus_uids,
            &sol.w.iter().map(|w| w.sqrt()).collect::<Vec<_>>(),
        )),
        va: None,
        w: Some(zip_bus(&acnet.bus_ids, &acnet.bus_uids, &sol.w)),
        wr: Some(
            acnet
                .branch_ids
                .iter()
                .zip(&sol.wr)
                .map(|(&branch, &value)| BranchScalar { branch, value })
                .collect(),
        ),
        wi: Some(
            acnet
                .branch_ids
                .iter()
                .zip(&sol.wi)
                .map(|(&branch, &value)| BranchScalar { branch, value })
                .collect(),
        ),
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
        dispatch: Some(zip_gen_pq(acnet, &sol.pg, &sol.qg, base)),
        constraints: None,
        sensitivities,
    })
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProblemCaps {
    pub formulation: Problem,
    /// Built in this binary (acopf is always `false`; it is not in this build).
    pub available: bool,
    /// Output blocks this formulation fills, e.g. `["lmp","va","flows","dispatch"]`.
    pub blocks: Vec<String>,
    /// The [`ConstraintTermKind`]s this formulation accepts in
    /// `SolveRequest.constraints`. Empty when it refuses linear constraints.
    #[serde(default)]
    pub constraints: Vec<ConstraintTermKind>,
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
            blocks: ["va", "flows"].map(str::to_owned).to_vec(),
            constraints: vec![],
            #[cfg(feature = "sensitivity")]
            operands: vec![],
            #[cfg(feature = "sensitivity")]
            parameters: vec![],
        },
        ProblemCaps {
            formulation: Problem::DcOpf,
            available: true,
            blocks: ["lmp", "va", "flows", "dispatch"]
                .map(str::to_owned)
                .to_vec(),
            constraints: vec![
                ConstraintTermKind::BranchFlow,
                ConstraintTermKind::BusInjection,
                ConstraintTermKind::Generator,
            ],
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
                Parameter::ConstraintLimit,
            ],
        },
        ProblemCaps {
            formulation: Problem::AcPf,
            available: cfg!(feature = "sensitivity"),
            blocks: ["vm", "va", "injections", "flows"]
                .map(str::to_owned)
                .to_vec(),
            constraints: vec![],
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
            blocks: ["lmp", "lmp_q", "vm", "w", "wr", "wi", "flows", "dispatch"]
                .map(str::to_owned)
                .to_vec(),
            constraints: vec![],
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
            blocks: ["lmp", "lmp_q", "vm", "va", "flows", "dispatch"]
                .map(str::to_owned)
                .to_vec(),
            constraints: vec![],
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
        Self::with_uids(ids, uids, needs_uids)
    }

    fn with_uids(ids: &[usize], uids: &'a [Option<String>], needs_uids: bool) -> KeyIndex<'a> {
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

fn aggregate_demand_deltas(
    deltas: &HashMap<ElementKey, f64>,
    idx: &KeyIndex<'_>,
) -> Result<BTreeMap<usize, (ElementKey, f64)>, String> {
    let mut aggregated = BTreeMap::<usize, (ElementKey, f64)>::new();
    for (bus, mw) in sorted_deltas(deltas) {
        if matches!(bus, ElementKey::Id(id) if *id <= 0) {
            return Err("demand delta bus must be positive".into());
        }
        if !mw.is_finite() {
            return Err(format!("demand delta for bus {bus} must be finite"));
        }
        let dense = idx
            .get(bus)
            .ok_or_else(|| format!("unknown demand delta bus {bus}"))?;
        let entry = aggregated
            .entry(dense)
            .or_insert_with(|| (bus.clone(), 0.0));
        entry.1 += mw;
        if !entry.1.is_finite() {
            return Err(format!(
                "aggregate demand delta for bus {bus} must be finite"
            ));
        }
    }
    Ok(aggregated)
}

/// Establish the operating point: `demand += delta` (per unit) at each named bus.
fn apply_demand_deltas(
    dc: &mut DcNetwork,
    deltas: &HashMap<ElementKey, f64>,
) -> Result<(), String> {
    let base = dc.base_mva;
    let idx = KeyIndex::new(&dc.bus_ids, &dc.bus_uids, deltas);
    for (i, (bus, mw)) in aggregate_demand_deltas(deltas, &idx)? {
        if dc.demand[i] * base + mw < -1e-9 {
            return Err(format!(
                "demand delta for bus {bus} would make demand negative"
            ));
        }
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
    for (i, (bus, mw)) in aggregate_demand_deltas(deltas, &idx)? {
        if acnet.pd[i] * base + mw < -1e-9 {
            return Err(format!(
                "demand delta for bus {bus} would make demand negative"
            ));
        }
        acnet.pd[i] += mw / base;
    }
    Ok(())
}

/// Validate one branch rating delta and resolve its branch to a dense index. Mirrors
/// Resolve and combine rating aliases before checking the final limit, so an ID
/// and UID naming the same row cannot make validation order-dependent.
fn aggregate_rating_deltas(
    rates: &HashMap<ElementKey, f64>,
    idx: &KeyIndex<'_>,
) -> Result<BTreeMap<usize, (ElementKey, f64)>, String> {
    let mut aggregated = BTreeMap::<usize, (ElementKey, f64)>::new();
    for (branch, mw) in sorted_deltas(rates) {
        if matches!(branch, ElementKey::Id(id) if *id <= 0) {
            return Err("rating delta branch must be positive".into());
        }
        if !mw.is_finite() {
            return Err(format!("rating delta for branch {branch} must be finite"));
        }
        let dense = idx
            .get(branch)
            .ok_or_else(|| format!("unknown rating delta branch {branch}"))?;
        let entry = aggregated
            .entry(dense)
            .or_insert_with(|| (branch.clone(), 0.0));
        entry.1 += mw;
        if !entry.1.is_finite() {
            return Err(format!(
                "aggregate rating delta for branch {branch} must be finite"
            ));
        }
    }
    Ok(aggregated)
}

/// Perturb the thermal limits: `fmax += delta` (per unit) at each named branch.
fn apply_rating_deltas(dc: &mut DcNetwork, rates: &HashMap<ElementKey, f64>) -> Result<(), String> {
    let base = dc.base_mva;
    let idx = KeyIndex::new(&dc.branch_ids, &dc.branch_uids, rates);
    for (i, (branch, mw)) in aggregate_rating_deltas(rates, &idx)? {
        if dc.fmax[i] * base + mw <= 1e-9 {
            return Err(format!(
                "rating delta for branch {branch} would make the line limit non-positive"
            ));
        }
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
    for (i, (branch, mw)) in aggregate_rating_deltas(rates, &idx)? {
        if acnet.rate_a[i] * base + mw <= 1e-9 {
            return Err(format!(
                "rating delta for branch {branch} would make the line limit non-positive"
            ));
        }
        acnet.rate_a[i] += mw / base;
    }
    Ok(())
}

/// Validate the request's linear constraints and resolve them onto the DC OPF
/// columns. Every term must name a source element of the solved network: an
/// unknown, out-of-service, or lowering-synthesized element is refused by name,
/// as is a constraint with no limit, crossed limits, a non-finite number, or
/// terms that cancel to nothing. A bus injection term expands onto the flows
/// of the bus's branches (`+` where the bus is the `from` end, `-` where it is
/// the `to` end), which equals generation minus demand and shunt withdrawal by
/// power balance, so the row does not move with demand and `nu_bal` stays the
/// marginal cost of demand.
fn apply_linear_constraints(
    dc: &mut DcNetwork,
    constraints: &[LinearConstraint],
) -> Result<(), String> {
    dc.linear_rows.clear();
    if constraints.is_empty() {
        return Ok(());
    }
    let needs_uids = constraints
        .iter()
        .flat_map(|constraint| &constraint.terms)
        .any(|term| matches!(term.element, ElementKey::Uid(_)));
    let buses = KeyIndex::with_uids(&dc.bus_ids, &dc.bus_uids, needs_uids);
    let branches = KeyIndex::with_uids(&dc.branch_ids, &dc.branch_uids, needs_uids);
    let generators = KeyIndex::with_uids(&dc.gen_ids, &dc.gen_uids, needs_uids);
    let mut incident: Option<Vec<Vec<(usize, f64)>>> = None;
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::with_capacity(constraints.len());
    for constraint in constraints {
        let id = &constraint.id;
        if id.is_empty() {
            return Err("constraint id must not be empty".into());
        }
        if !seen.insert(id.as_str()) {
            return Err(format!("duplicate constraint id \"{id}\""));
        }
        match (constraint.lower, constraint.upper) {
            (None, None) => {
                return Err(format!(
                    "constraint \"{id}\" needs a lower limit, an upper limit, or both"
                ));
            }
            (lower, upper)
                if lower.is_some_and(|v| !v.is_finite())
                    || upper.is_some_and(|v| !v.is_finite()) =>
            {
                return Err(format!("constraint \"{id}\" limits must be finite"));
            }
            (Some(lower), Some(upper)) if lower > upper => {
                return Err(format!(
                    "constraint \"{id}\" lower limit {lower} exceeds its upper limit {upper}"
                ));
            }
            _ => {}
        }
        if constraint.terms.is_empty() {
            return Err(format!("constraint \"{id}\" has no terms"));
        }
        let mut flow = BTreeMap::<usize, f64>::new();
        let mut generation = BTreeMap::<usize, f64>::new();
        for term in &constraint.terms {
            let key = &term.element;
            if !term.coefficient.is_finite() {
                return Err(format!(
                    "constraint \"{id}\" coefficient for {key} must be finite"
                ));
            }
            match term.kind {
                ConstraintTermKind::BranchFlow => {
                    let e = branches
                        .get(key)
                        .filter(|&e| dc.branch_source_rows[e].is_some())
                        .ok_or_else(|| {
                            format!("constraint \"{id}\": unknown or out-of-service branch {key}")
                        })?;
                    *flow.entry(e).or_default() += term.coefficient;
                }
                ConstraintTermKind::BusInjection => {
                    let i = buses
                        .get(key)
                        .filter(|&i| dc.bus_source_rows[i].is_some())
                        .ok_or_else(|| {
                            format!("constraint \"{id}\": unknown or out-of-service bus {key}")
                        })?;
                    let incident = incident.get_or_insert_with(|| {
                        let mut at = vec![Vec::new(); dc.n];
                        for e in 0..dc.m {
                            at[dc.br_from[e]].push((e, 1.0));
                            at[dc.br_to[e]].push((e, -1.0));
                        }
                        at
                    });
                    for &(e, sign) in &incident[i] {
                        *flow.entry(e).or_default() += sign * term.coefficient;
                    }
                }
                ConstraintTermKind::Generator => {
                    if let ElementKey::Uid(uid) = key {
                        let named = dc
                            .gen_uids
                            .iter()
                            .filter(|candidate| candidate.as_deref() == Some(uid.as_str()))
                            .count();
                        if named > 1 {
                            return Err(format!(
                                "constraint \"{id}\": generator uid {key} names {named} generators"
                            ));
                        }
                    }
                    let j = generators
                        .get(key)
                        .filter(|&j| dc.gen_source_rows[j].is_some())
                        .ok_or_else(|| {
                            format!(
                                "constraint \"{id}\": unknown or out-of-service generator {key}"
                            )
                        })?;
                    *generation.entry(j).or_default() += term.coefficient;
                }
            }
        }
        let nonzero = |terms: BTreeMap<usize, f64>| -> Result<Vec<(usize, f64)>, String> {
            let terms: Vec<(usize, f64)> = terms.into_iter().filter(|&(_, c)| c != 0.0).collect();
            if terms.iter().any(|&(_, c)| !c.is_finite()) {
                return Err(format!(
                    "constraint \"{id}\" combined coefficients must be finite"
                ));
            }
            Ok(terms)
        };
        let flow = nonzero(flow)?;
        let generation = nonzero(generation)?;
        if flow.is_empty() && generation.is_empty() {
            return Err(format!(
                "constraint \"{id}\" has no nonzero coefficient once its terms are combined"
            ));
        }
        rows.push(super::model::LinearRow {
            id: id.clone(),
            flow,
            generation,
            lower_mw: constraint.lower,
            upper_mw: constraint.upper,
        });
    }
    dc.linear_rows = rows;
    Ok(())
}

/// The response block for the solved model's linear constraints. `economic`
/// gates the shadow price exactly as it gates `lmp`.
fn linear_constraint_results(
    dc: &DcNetwork,
    sol: &super::problem::DcOpfSolution,
    economic: bool,
) -> Vec<ConstraintResult> {
    let base = dc.base_mva;
    dc.linear_rows
        .iter()
        .zip(sol.linear_values.iter().zip(&sol.linear_duals))
        .map(|(row, (&value, &dual))| {
            let value = value * base;
            // The interior point solve meets an active limit to about its 1e-9
            // per unit feasibility tolerance; allow a margin above that.
            let at = |limit: Option<f64>| {
                limit.is_some_and(|limit| (value - limit).abs() <= 1e-6 * (1.0 + limit.abs()))
            };
            ConstraintResult {
                id: row.id.clone(),
                value,
                lower: row.lower_mw,
                upper: row.upper_mw,
                shadow_price: economic.then_some(dual / base),
                binding: row.is_equality() || at(row.lower_mw) || at(row.upper_mw),
            }
        })
        .collect()
}

/// Linear constraints are a DC OPF feature; refuse them anywhere else instead
/// of silently solving without them.
#[cfg(feature = "sensitivity")]
fn reject_linear_constraints(
    constraints: &[LinearConstraint],
    formulation: &str,
) -> Result<(), String> {
    if constraints.is_empty() {
        return Ok(());
    }
    Err(format!(
        "linear constraints are supported only by dcopf, not {formulation}"
    ))
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

fn zip_gen_pg(network: &DcNetwork, pg: &[f64], base: f64) -> Vec<GenDispatch> {
    network
        .gen_ids
        .iter()
        .zip(pg)
        .enumerate()
        .map(|(j, (&gen, &p))| GenDispatch {
            gen,
            bus: Some(network.bus_ids[network.gen_bus[j]]),
            pg: p * base,
            qg: None,
        })
        .collect()
}

#[cfg(feature = "conic")]
fn zip_gen_pq(
    network: &crate::model::AcNetwork,
    pg: &[f64],
    qg: &[f64],
    base: f64,
) -> Vec<GenDispatch> {
    network
        .gen_ids
        .iter()
        .enumerate()
        .map(|(j, &gen)| GenDispatch {
            gen,
            bus: Some(network.bus_ids[network.gen_bus[j]]),
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
#[cfg(feature = "sensitivity")]
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
        serde_json::to_string(&crate::model::parse_matpower(CASE3).expect("parse"))
            .expect("network JSON")
    }

    fn case3_module_json() -> String {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let module = powerio::PioModule::new(powerio::PioValue::BalancedNetwork(network));
        crate::ir::serialize_module(&module).expect("module JSON")
    }

    #[test]
    fn module_entry_solves_a_balanced_network_and_rejects_bare_model_json() {
        let response = solve_module_json(&case3_module_json(), "{}").expect("module solve");
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["formulation"], "dcopf");
        assert_eq!(response["status"], "optimal");
        let buses: Vec<_> = response["dispatch"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["bus"].as_u64().unwrap())
            .collect();
        assert_eq!(buses, vec![1, 3]);

        let error = solve_module_json(&case3_json(), "{}")
            .expect_err("bare model JSON is not a portable solve input");
        assert!(!error.is_empty());
    }

    fn case3_with_outages_json() -> String {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        net.branches_mut()[0].in_service = false;
        net.generators_mut()[0].in_service = false;
        serde_json::to_string(&net).expect("network JSON")
    }

    #[test]
    fn empty_request_defaults_to_dc_opf() {
        // `{}` and `""` both deserialize to a base-case DC OPF.
        for body in ["", "{}"] {
            let out = solve_test_network_json(&case3_json(), body).expect("solve");
            let v: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["formulation"], "dcopf");
            assert_eq!(v["status"], "optimal");
        }
    }

    #[test]
    fn retired_solver_options_are_rejected() {
        let error = solve_test_network_json(
            &case3_json(),
            r#"{"formulation":"dcopf","options":{"shed":true}}"#,
        )
        .expect_err("retired solver options must not be ignored");
        assert!(error.contains("unknown field `options`"), "{error}");
    }

    #[test]
    fn feasibility_instance_omits_prices() {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let instance = DcOpfInstance::from_network(network)
            .expect("instance")
            .with_objective(powerio_prob::Objective::none());
        let response = solve_instance(&instance, &SolveRequest::default()).expect("solve");
        assert_eq!(response.objective, Some(0.0));
        assert!(response.lmp.is_none());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn feasibility_instance_rejects_price_sensitivity() {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let instance = DcOpfInstance::from_network(network)
            .expect("instance")
            .with_objective(powerio_prob::Objective::none());
        let request: SolveRequest = serde_json::from_str(
            r#"{"sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"}}]}"#,
        )
        .expect("request");
        let error = solve_instance(&instance, &request).unwrap_err();
        assert!(error.contains("network_generator_cost"), "{error}");
    }

    #[cfg(feature = "conic")]
    #[test]
    fn typed_ac_instance_uses_the_socwr_entry() {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let instance = AcOpfInstance::from_network(network).expect("instance");
        let request = SolveRequest {
            formulation: Problem::Socwr,
            ..SolveRequest::default()
        };
        let response = solve_ac_instance(&instance, &request).expect("solve");
        assert!(response.objective.is_some_and(|value| value > 0.0));
        assert_eq!(response.lmp.as_ref().map(Vec::len), Some(3));
    }

    #[test]
    fn dc_opf_payload_shapes() {
        let out =
            solve_test_network_json(&case3_json(), r#"{"formulation":"dcopf"}"#).expect("solve");
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
        let base: Value = serde_json::from_str(
            &solve_test_network_json(&case3_json(), r#"{"formulation":"dcopf"}"#).unwrap(),
        )
        .unwrap();
        let bumped: Value = serde_json::from_str(
            &solve_test_network_json(
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
        let err = solve_test_network_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"999":1.0}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown demand delta bus 999"), "got: {err}");
    }

    #[test]
    fn demand_delta_cannot_make_demand_negative() {
        let err = solve_test_network_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"2":-1000.0}}}"#,
        )
        .unwrap_err();
        assert!(
            err.contains("demand delta for bus 2 would make demand negative"),
            "got: {err}"
        );
    }

    /// CASE3 with explicit PowerIO row UIDs for keyed edit coverage.
    fn case3_with_uids_json() -> String {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        for (i, b) in net.buses_mut().iter_mut().enumerate() {
            b.uid = Some(format!("buses:{i}"));
        }
        for (i, br) in net.branches_mut().iter_mut().enumerate() {
            br.uid = Some(format!("branches:{i}"));
        }
        serde_json::to_string(&net).expect("network JSON")
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
            &solve_test_network_json(
                &net,
                r#"{"formulation":"dcopf","edits":{"deltas":{"2":50.0},"rates":{"3":-25.0}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        let by_uid: Value = serde_json::from_str(
            &solve_test_network_json(
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
        let err = solve_test_network_json(
            &case3_with_uids_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"buses:99":1.0}}}"#,
        )
        .unwrap_err();
        assert!(
            err.contains(r#"unknown demand delta bus "buses:99""#),
            "got: {err}"
        );
        // A network that was never stamped resolves no uid key at all.
        let err = solve_test_network_json(
            &case3_json(),
            r#"{"formulation":"dcopf","edits":{"deltas":{"buses:1":1.0}}}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown demand delta bus"), "got: {err}");
    }

    #[test]
    fn duplicate_canonical_uids_reject_even_an_unedited_solve() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        net.buses_mut()[0].uid = Some("same-bus".into());
        net.buses_mut()[1].uid = Some("same-bus".into());
        let error = solve_network(&net, &SolveRequest::default()).unwrap_err();
        assert!(error.contains("duplicate bus uid"), "{error}");

        net.buses_mut()[1].uid = Some("different-bus".into());
        net.branches_mut()[0].uid = Some("same-branch".into());
        net.branches_mut()[1].uid = Some("same-branch".into());
        let error = solve_network(&net, &SolveRequest::default()).unwrap_err();
        assert!(error.contains("duplicate branch uid"), "{error}");
    }

    #[test]
    fn numeric_looking_uids_reject_before_key_resolution() {
        for uid in ["2", "02", "+2", "-2", "9007199254740993"] {
            let mut net = crate::model::parse_matpower(CASE3).expect("parse");
            net.buses_mut()[0].uid = Some(uid.into());
            let error = solve_network(&net, &SolveRequest::default()).unwrap_err();
            assert!(
                error.contains("ambiguous with a numeric element id"),
                "{error}"
            );
        }
    }

    #[test]
    #[cfg(feature = "sensitivity")]
    fn acpf_rejects_remote_generator_voltage_control() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        net.generators_mut()[0].regulated_bus = Some(powerio::BusId(2));
        let request = SolveRequest {
            formulation: Problem::AcPf,
            ..SolveRequest::default()
        };
        let error = solve_network(&net, &request).unwrap_err();
        assert!(error.contains("regulating a remote bus"), "{error}");
    }

    #[test]
    fn id_and_uid_aliases_are_aggregated_before_bounds() {
        let network = case3_with_uids_json();
        let base: Value = serde_json::from_str(
            &solve_test_network_json(&network, r#"{"formulation":"dcopf"}"#).unwrap(),
        )
        .unwrap();
        let cancelled: Value = serde_json::from_str(
            &solve_test_network_json(
                &network,
                r#"{"formulation":"dcopf","edits":{"deltas":{"2":-1000.0,"buses:1":1000.0},"rates":{"1":-1000.0,"branches:0":1000.0}}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(base["objective"], cancelled["objective"]);
        assert_eq!(base["flows"], cancelled["flows"]);

        for edits in [
            r#"{"deltas":{"2":-1000.0,"buses:1":1.0}}"#,
            r#"{"deltas":{"2":1.0,"buses:1":-1000.0}}"#,
        ] {
            let request = format!(r#"{{"formulation":"dcopf","edits":{edits}}}"#);
            let error = solve_test_network_json(&network, &request).unwrap_err();
            assert!(error.contains("would make demand negative"), "{error}");
        }
        for edits in [
            r#"{"rates":{"1":-100000.0,"branches:0":1.0}}"#,
            r#"{"rates":{"1":1.0,"branches:0":-100000.0}}"#,
        ] {
            let request = format!(r#"{{"formulation":"dcopf","edits":{edits}}}"#);
            let error = solve_test_network_json(&network, &request).unwrap_err();
            assert!(error.contains("line limit non-positive"), "{error}");
        }
    }

    #[test]
    fn response_scalars_echo_uids() {
        let out =
            solve_test_network_json(&case3_with_uids_json(), r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["lmp"][1]["uid"], "buses:1");
        assert_eq!(v["va"][0]["uid"], "buses:0");
        assert_eq!(v["flows"][0]["uid"], "branches:0");
        // A network whose source supplied no uids carries the ones PowerIO
        // assigned: the bus number and the branch terminals.
        let out = solve_test_network_json(&case3_json(), r#"{"formulation":"dcopf"}"#).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["lmp"][1]["uid"], "2");
        assert_eq!(v["flows"][0]["uid"], "1-2");
    }

    #[test]
    fn payload_ids_survive_out_of_service_elements() {
        let out = solve_test_network_json(&case3_with_outages_json(), r#"{"formulation":"dcopf"}"#)
            .expect("solve");
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
    fn portable_solve_disables_undeclared_load_shedding() {
        // Generation capacity (0.8 pu) below the 0.9 pu load is infeasible under the
        // declared PowerIO instance. The public response cannot hide a private shed.
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        for generator in net.generators_mut() {
            generator.pmax = 40.0;
        }
        let instance = DcOpfInstance::from_network(net).expect("instance");

        let result = solve_instance(&instance, &SolveRequest::default());
        assert!(
            result.is_err(),
            "expected infeasible without shedding, got {result:?}"
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
        let out = solve_test_network_json(&case3_json(), req).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        let sens = v["sensitivities"].as_array().unwrap();
        assert_eq!(sens.len(), 1);
        let m = &sens[0];
        assert_eq!(m["units"], "(objective_unit/MW)/MW");
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
        let err = solve_test_network_json(&case3_json(), req).unwrap_err();
        assert!(err.contains("does not support"), "got: {err}");
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn ac_pf_reports_voltages_and_injections() {
        let out =
            solve_test_network_json(&case3_json(), r#"{"formulation":"acpf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "acpf");
        assert_eq!(v["vm"].as_array().unwrap().len(), 3);
        assert_eq!(v["va"].as_array().unwrap().len(), 3);
        assert_eq!(v["injections"].as_array().unwrap().len(), 3);
        assert!(v["lmp"].is_null());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn typed_ac_pf_instance_uses_its_declared_bus_specifications() {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let instance = AcPfInstance::from_network(network).expect("instance");
        let request = SolveRequest {
            formulation: Problem::AcPf,
            ..Default::default()
        };
        let response = solve_ac_pf_instance(&instance, &request).expect("typed AC PF");
        assert_eq!(response.formulation, Problem::AcPf);
        assert_eq!(response.vm.as_ref().unwrap().len(), 3);
        assert_eq!(response.injections.as_ref().unwrap().len(), 3);
        assert!(response.lmp.is_none());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn dc_pf_reports_angles_and_flows() {
        let out =
            solve_test_network_json(&case3_json(), r#"{"formulation":"dcpf"}"#).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "dcpf");
        assert_eq!(v["va"].as_array().unwrap().len(), 3);
        assert_eq!(v["flows"].as_array().unwrap().len(), 3);
        assert!(v["lmp"].is_null());
        assert!(v["dispatch"].is_null());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn dc_pf_served_results_match_for_raw_and_normalized_inputs() {
        let raw = crate::model::parse_matpower(CASE3).expect("parse case3");
        let normalized = raw.to_normalized().expect("normalize case3");
        let request = SolveRequest {
            formulation: Problem::DcPf,
            ..Default::default()
        };
        let a = solve_network(&raw, &request).expect("solve raw DCPF");
        let b = solve_network(&normalized, &request).expect("solve normalized DCPF");

        let a_va = a.va.as_ref().expect("raw angles");
        let b_va = b.va.as_ref().expect("normalized angles");
        assert_eq!(a_va.len(), b_va.len());
        for (left, right) in a_va.iter().zip(b_va) {
            assert_eq!(left.bus, right.bus);
            assert!((left.value - right.value).abs() < 1e-8);
        }
        let a_flows = a.flows.as_ref().expect("raw flows");
        let b_flows = b.flows.as_ref().expect("normalized flows");
        assert_eq!(a_flows.len(), b_flows.len());
        for (left, right) in a_flows.iter().zip(b_flows) {
            assert_eq!(left.branch, right.branch);
            assert!((left.pf - right.pf).abs() < 1e-8);
        }
    }

    #[cfg(feature = "conic")]
    #[test]
    fn socwr_reports_w_and_reactive_capable_sensitivity() {
        let req = r#"{"formulation":"socwr","sensitivities":[{"operand":{"Price":"Reactive"},"parameter":{"Demand":"Active"}}]}"#;
        let out = solve_test_network_json(&case3_json(), req).expect("solve");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["formulation"], "socwr");
        assert_eq!(v["w"].as_array().unwrap().len(), 3);
        let m = &v["sensitivities"].as_array().unwrap()[0];
        assert_eq!(m["units"], "(objective_unit/MVAr)/MW");
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
        let err = solve_test_network_json(&case3_json(), r#"{"formulation":"acopf"}"#).unwrap_err();
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

        let net = crate::model::parse_matpower(CASE3).unwrap();
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

    /// CASE3 with linear costs ($10/MWh at bus 1, $20/MWh at bus 3) and zero
    /// minimum output, so every dispatch and dual below is exact by hand. The
    /// three lines are identical, so a MW injected at one bus and withdrawn at
    /// another splits 2/3 on the direct line and 1/3 around the other two.
    fn linear_cost_case3_json() -> String {
        let text = CASE3
            .replace(" 1 100 1 250 10 ", " 1 100 1 250 0 ")
            .replace(" 1 100 1 270 10 ", " 1 100 1 270 0 ")
            .replace(" 2 0 0 3 0.11  5   0;", " 2 0 0 2 10 0;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 2 0 0 2 20 0;");
        serde_json::to_string(&crate::model::parse_matpower(&text).expect("parse"))
            .expect("network JSON")
    }

    fn solve_constrained(network: &str, constraints: Value) -> SolveResponse {
        let request = serde_json::json!({ "formulation": "dcopf", "constraints": constraints });
        let out = solve_test_network_json(network, &request.to_string()).expect("solve");
        serde_json::from_str(&out).expect("response")
    }

    fn close(actual: f64, expected: f64, what: &str) {
        assert!(
            (actual - expected).abs() < 1e-4,
            "{what}: expected {expected}, got {actual}"
        );
    }

    fn prices(response: &SolveResponse) -> Vec<f64> {
        response
            .lmp
            .as_ref()
            .expect("lmp")
            .iter()
            .map(|p| p.value)
            .collect()
    }

    fn dispatch(response: &SolveResponse) -> Vec<f64> {
        response
            .dispatch
            .as_ref()
            .expect("dispatch")
            .iter()
            .map(|g| g.pg)
            .collect()
    }

    #[test]
    fn interface_limit_shadow_price_matches_the_hand_computation() {
        let network = linear_cost_case3_json();
        // Unconstrained, the $10 unit serves all 90 MW and every price is $10.
        let free = solve_constrained(&network, serde_json::json!([]));
        assert!(free.constraints.is_none());
        for price in prices(&free) {
            close(price, 10.0, "unconstrained price");
        }

        // Line 1-2 carries 30 + g1/3 MW, so a 50 MW limit holds g1 to 60 MW and
        // g3 takes the other 30 MW. Each MW of limit lets 3 MW move from the $20
        // unit to the $10 unit: the shadow price is $30/MW.
        let response = solve_constrained(
            &network,
            serde_json::json!([{
                "id": "line 1-2",
                "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
                "upper": 50.0
            }]),
        );
        let pg = dispatch(&response);
        close(pg[0], 60.0, "g1");
        close(pg[1], 30.0, "g3");
        close(response.objective.unwrap(), 1200.0, "objective");
        let row = &response.constraints.as_ref().expect("constraints")[0];
        assert_eq!(row.id, "line 1-2");
        assert_eq!(row.upper, Some(50.0));
        assert_eq!(row.lower, None);
        close(row.value, 50.0, "interface value");
        close(row.shadow_price.unwrap(), 30.0, "shadow price");
        assert!(row.binding);

        // Bus 2's price is the cheapest way to serve one more MW there without
        // loading line 1-2: -1 MW at bus 1 and +2 MW at bus 3, $30.
        let lmp = prices(&response);
        close(lmp[0], 10.0, "lmp bus 1");
        close(lmp[1], 30.0, "lmp bus 2");
        close(lmp[2], 20.0, "lmp bus 3");
        // Decomposition against bus 1: a MW injected at bus 2 (bus 3) and
        // withdrawn at bus 1 changes the line 1-2 flow by -2/3 (-1/3) MW.
        let shadow = row.shadow_price.unwrap();
        close(
            lmp[1],
            lmp[0] - shadow * (-2.0 / 3.0),
            "bus 2 decomposition",
        );
        close(
            lmp[2],
            lmp[0] - shadow * (-1.0 / 3.0),
            "bus 3 decomposition",
        );
    }

    #[test]
    fn a_slack_constraint_reports_a_zero_shadow_price() {
        let response = solve_constrained(
            &linear_cost_case3_json(),
            serde_json::json!([{
                "id": "loose",
                "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
                "lower": -200.0,
                "upper": 200.0
            }]),
        );
        let row = &response.constraints.unwrap()[0];
        // g1 = 90 MW puts 30 + 90/3 = 60 MW on line 1-2.
        close(row.value, 60.0, "value");
        close(row.shadow_price.unwrap(), 0.0, "shadow price");
        assert!(!row.binding);
    }

    #[test]
    fn one_sided_lower_limit_on_generator_output() {
        // At least 40 MW from the $20 unit costs $10 per MW of requirement. The
        // network is uncongested, so every bus keeps the $10 price.
        let response = solve_constrained(
            &linear_cost_case3_json(),
            serde_json::json!([{
                "id": "bus 3 minimum",
                "terms": [{ "kind": "generator", "element": 2, "coefficient": 1.0 }],
                "lower": 40.0
            }]),
        );
        let pg = dispatch(&response);
        close(pg[0], 50.0, "g1");
        close(pg[1], 40.0, "g3");
        let row = &response.constraints.as_ref().unwrap()[0];
        close(row.value, 40.0, "value");
        close(row.shadow_price.unwrap(), -10.0, "shadow price");
        assert!(row.binding);
        for price in prices(&response) {
            close(price, 10.0, "price");
        }
    }

    #[test]
    fn two_sided_bus_injection_limit_shifts_its_bus_price_by_coefficient_times_shadow() {
        let network = linear_cost_case3_json();
        // Bus 3 has no load, so its net injection is g3. Holding it within
        // [25, 100] MW binds at the lower limit. A MW injected at bus 3 and
        // withdrawn at bus 1 moves the row's value by its coefficient, so bus 3's
        // price sits `-coefficient * shadow` above bus 1's.
        for coefficient in [1.0, 2.0] {
            let response = solve_constrained(
                &network,
                serde_json::json!([{
                    "id": "bus 3 band",
                    "terms": [{
                        "kind": "bus_injection",
                        "element": 3,
                        "coefficient": coefficient
                    }],
                    "lower": 25.0 * coefficient,
                    "upper": 100.0 * coefficient
                }]),
            );
            let pg = dispatch(&response);
            close(pg[1], 25.0, "g3");
            let row = &response.constraints.as_ref().unwrap()[0];
            close(row.value, 25.0 * coefficient, "value");
            close(
                row.shadow_price.unwrap(),
                -10.0 / coefficient,
                "shadow price",
            );
            assert!(row.binding);
            let lmp = prices(&response);
            close(lmp[0], 10.0, "lmp bus 1");
            close(lmp[1], 10.0, "lmp bus 2");
            close(
                lmp[2],
                lmp[0] - coefficient * row.shadow_price.unwrap(),
                "lmp bus 3",
            );
            close(lmp[2], 20.0, "lmp bus 3");
        }

        // The upper side: bus 1's net injection held to [0, 60] MW. Against bus 2,
        // bus 1's price sits one shadow price lower.
        let response = solve_constrained(
            &network,
            serde_json::json!([{
                "id": "bus 1 band",
                "terms": [{ "kind": "bus_injection", "element": 1, "coefficient": 1.0 }],
                "lower": 0.0,
                "upper": 60.0
            }]),
        );
        let row = &response.constraints.as_ref().unwrap()[0];
        close(row.value, 60.0, "value");
        close(row.shadow_price.unwrap(), 10.0, "shadow price");
        let lmp = prices(&response);
        close(lmp[1], 20.0, "lmp bus 2");
        close(lmp[2], 20.0, "lmp bus 3");
        close(lmp[0], lmp[1] - row.shadow_price.unwrap(), "lmp bus 1");
    }

    #[test]
    fn equality_constraint_fixes_its_value() {
        let response = solve_constrained(
            &linear_cost_case3_json(),
            serde_json::json!([{
                "id": "fixed",
                "terms": [{ "kind": "generator", "element": 2, "coefficient": 1.0 }],
                "lower": 35.0,
                "upper": 35.0
            }]),
        );
        let pg = dispatch(&response);
        close(pg[0], 55.0, "g1");
        close(pg[1], 35.0, "g3");
        let row = &response.constraints.as_ref().unwrap()[0];
        close(row.value, 35.0, "value");
        close(row.shadow_price.unwrap(), -10.0, "shadow price");
        assert!(row.binding);
    }

    #[test]
    fn several_constraints_report_in_request_order() {
        let response = solve_constrained(
            &linear_cost_case3_json(),
            serde_json::json!([
                {
                    "id": "b",
                    "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
                    "upper": 50.0
                },
                {
                    "id": "a",
                    "terms": [
                        { "kind": "generator", "element": 1, "coefficient": 1.0 },
                        { "kind": "generator", "element": 2, "coefficient": 1.0 }
                    ],
                    "lower": 0.0
                }
            ]),
        );
        let rows = response.constraints.unwrap();
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        close(rows[0].shadow_price.unwrap(), 30.0, "interface");
        close(rows[1].value, 90.0, "total generation");
        close(rows[1].shadow_price.unwrap(), 0.0, "slack row");
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn interface_shadow_price_decomposes_the_case9_prices() {
        let network = serde_json::to_string(
            &crate::model::parse_matpower(crate::model::CASE9).expect("parse case9"),
        )
        .unwrap();
        // An interface over lines 4-5 and 6-7 (branches 2 and 5), held 15 MW
        // below its unconstrained value so it binds alone.
        let terms = serde_json::json!([
            { "kind": "branch_flow", "element": 2, "coefficient": 1.0 },
            { "kind": "branch_flow", "element": 5, "coefficient": 1.0 }
        ]);
        let base = solve_constrained(
            &network,
            serde_json::json!([{ "id": "probe", "terms": terms, "lower": -1.0e4 }]),
        );
        let v0 = base.constraints.unwrap()[0].value;
        let limit = if v0 >= 0.0 {
            serde_json::json!({ "upper": v0 - 15.0 })
        } else {
            serde_json::json!({ "lower": v0 + 15.0 })
        };
        let mut constraint = serde_json::json!({ "id": "interface", "terms": terms });
        constraint
            .as_object_mut()
            .unwrap()
            .extend(limit.as_object().unwrap().clone());
        let response = solve_constrained(&network, serde_json::json!([constraint]));
        let row = &response.constraints.as_ref().unwrap()[0];
        assert!(row.binding);
        let shadow = row.shadow_price.unwrap();
        assert!(
            shadow.abs() > 1e-3,
            "the interface must bind, shadow {shadow}"
        );
        for flow in response.flows.as_ref().unwrap() {
            assert!(flow.loading < 0.999, "branch {} binds", flow.branch);
        }

        // Shift factors from DC power flow: a MW of demand at bus i is a MW
        // withdrawn at i and injected at the slack, bus 1.
        let flows = |deltas: Value| -> Vec<f64> {
            let request =
                serde_json::json!({ "formulation": "dcpf", "edits": { "deltas": deltas } });
            let out = solve_test_network_json(&network, &request.to_string()).expect("dcpf");
            let response: SolveResponse = serde_json::from_str(&out).unwrap();
            response.flows.unwrap().iter().map(|f| f.pf).collect()
        };
        let reference = flows(serde_json::json!({}));
        let lmp = prices(&response);
        for (i, price) in lmp.iter().enumerate() {
            let shifted = flows(serde_json::json!({ (i + 1).to_string(): 1.0 }));
            // Interface value per MW injected at bus i and withdrawn at bus 1.
            let shift = -((shifted[1] - reference[1]) + (shifted[4] - reference[4]));
            close(*price, lmp[0] - shadow * shift, &format!("bus {}", i + 1));
        }
    }

    #[test]
    fn uid_keyed_terms_match_id_keyed_terms() {
        // The row is 30 + g1/3 + 0.5 g3 = 75 - g1/6 MW, so 65 MW requires g1 >= 60
        // MW against an unconstrained g1 of about 30 MW.
        let by_id = solve_constrained(
            &case3_with_uids_json(),
            serde_json::json!([{
                "id": "x",
                "terms": [
                    { "kind": "branch_flow", "element": 1, "coefficient": 1.0 },
                    { "kind": "bus_injection", "element": 3, "coefficient": 0.5 }
                ],
                "upper": 65.0
            }]),
        );
        let by_uid = solve_constrained(
            &case3_with_uids_json(),
            serde_json::json!([{
                "id": "x",
                "terms": [
                    { "kind": "branch_flow", "element": "branches:0", "coefficient": 1.0 },
                    { "kind": "bus_injection", "element": "buses:2", "coefficient": 0.5 }
                ],
                "upper": 65.0
            }]),
        );
        let (a, b) = (
            &by_id.constraints.unwrap()[0],
            &by_uid.constraints.unwrap()[0],
        );
        assert!(a.binding && a.shadow_price.unwrap() > 1e-3);
        close(a.value, 65.0, "value");
        close(a.value, b.value, "value");
        close(
            a.shadow_price.unwrap(),
            b.shadow_price.unwrap(),
            "shadow price",
        );
    }

    #[test]
    fn feasibility_instance_omits_constraint_shadow_prices() {
        let network = crate::model::parse_matpower(CASE3).expect("parse");
        let instance = DcOpfInstance::from_network(network)
            .expect("instance")
            .with_objective(powerio_prob::Objective::none());
        let request: SolveRequest = serde_json::from_value(serde_json::json!({
            "constraints": [{
                "id": "x",
                "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
                "upper": 40.0
            }]
        }))
        .unwrap();
        let response = solve_instance(&instance, &request).expect("solve");
        let row = &response.constraints.unwrap()[0];
        assert!(row.value <= 40.0 + 1e-6);
        assert!(row.shadow_price.is_none());
    }

    #[test]
    fn unknown_or_unusable_constraint_elements_are_refused() {
        let network = case3_json();
        let refused = |term: Value| {
            let request = serde_json::json!({
                "constraints": [{ "id": "probe", "terms": [term], "upper": 10.0 }]
            });
            solve_test_network_json(&network, &request.to_string())
                .expect_err("an unknown element must be refused")
        };
        for (term, expected) in [
            (
                serde_json::json!({ "kind": "branch_flow", "element": 99, "coefficient": 1.0 }),
                "constraint \"probe\": unknown or out-of-service branch 99",
            ),
            (
                serde_json::json!({ "kind": "bus_injection", "element": 7, "coefficient": 1.0 }),
                "constraint \"probe\": unknown or out-of-service bus 7",
            ),
            (
                serde_json::json!({ "kind": "generator", "element": 3, "coefficient": 1.0 }),
                "constraint \"probe\": unknown or out-of-service generator 3",
            ),
            (
                serde_json::json!({ "kind": "branch_flow", "element": "branches:0", "coefficient": 1.0 }),
                "constraint \"probe\": unknown or out-of-service branch \"branches:0\"",
            ),
            (
                serde_json::json!({ "kind": "generator", "element": -1, "coefficient": 1.0 }),
                "constraint \"probe\": unknown or out-of-service generator -1",
            ),
        ] {
            assert_eq!(refused(term), expected);
        }

        // An out-of-service branch and generator are not in the solved network.
        let outaged = case3_with_outages_json();
        for term in [
            serde_json::json!({ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }),
            serde_json::json!({ "kind": "generator", "element": 1, "coefficient": 1.0 }),
        ] {
            let request = serde_json::json!({
                "constraints": [{ "id": "probe", "terms": [term], "upper": 10.0 }]
            });
            let error = solve_test_network_json(&outaged, &request.to_string())
                .expect_err("an out-of-service element must be refused");
            assert!(error.contains("unknown or out-of-service"), "{error}");
        }

        let error = solve_test_network_json(
            &network,
            r#"{"constraints":[{"id":"x","terms":[{"kind":"line","element":1,"coefficient":1}],"upper":1}]}"#,
        )
        .expect_err("an unknown term kind must be refused");
        assert!(error.contains("unknown variant `line`"), "{error}");
    }

    #[test]
    fn malformed_constraints_are_refused() {
        let network = case3_json();
        let term = serde_json::json!({ "kind": "branch_flow", "element": 1, "coefficient": 1.0 });
        for (constraints, expected) in [
            (
                serde_json::json!([{ "id": "x", "terms": [term] }]),
                "constraint \"x\" needs a lower limit, an upper limit, or both",
            ),
            (
                serde_json::json!([{ "id": "x", "terms": [term], "lower": 5.0, "upper": 1.0 }]),
                "constraint \"x\" lower limit 5 exceeds its upper limit 1",
            ),
            (
                serde_json::json!([{ "id": "x", "terms": [], "upper": 1.0 }]),
                "constraint \"x\" has no terms",
            ),
            (
                serde_json::json!([{ "id": "", "terms": [term], "upper": 1.0 }]),
                "constraint id must not be empty",
            ),
            (
                serde_json::json!([
                    { "id": "x", "terms": [term], "upper": 1.0 },
                    { "id": "x", "terms": [term], "upper": 2.0 }
                ]),
                "duplicate constraint id \"x\"",
            ),
            (
                serde_json::json!([{
                    "id": "x",
                    "terms": [
                        term,
                        { "kind": "branch_flow", "element": 1, "coefficient": -1.0 }
                    ],
                    "upper": 1.0
                }]),
                "constraint \"x\" has no nonzero coefficient once its terms are combined",
            ),
        ] {
            let request = serde_json::json!({ "constraints": constraints });
            let error = solve_test_network_json(&network, &request.to_string())
                .expect_err("a malformed constraint must be refused");
            assert_eq!(error, expected);
        }

        let error = solve_test_network_json(
            &network,
            r#"{"constraints":[{"id":"x","terms":[],"limit":1}]}"#,
        )
        .expect_err("an unknown constraint field must be refused");
        assert!(error.contains("unknown field `limit`"), "{error}");
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn constraints_are_refused_outside_dc_opf() {
        let constraints = serde_json::json!([{
            "id": "x",
            "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
            "upper": 50.0
        }]);
        let mut formulations = vec!["dcpf", "acpf"];
        if cfg!(feature = "conic") {
            formulations.push("socwr");
        }
        for formulation in formulations {
            let request =
                serde_json::json!({ "formulation": formulation, "constraints": constraints });
            let error = solve_test_network_json(&case3_json(), &request.to_string())
                .expect_err("only dcopf accepts linear constraints");
            assert_eq!(
                error,
                format!("linear constraints are supported only by dcopf, not {formulation}")
            );
        }
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn constraint_limit_sensitivity_matches_the_hand_computed_prices() {
        // The 50 MW interface on line 1-2 of the linear-cost case: each MW of limit
        // moves 3 MW from the $20 unit to the $10 unit (objective -$30, the shadow
        // price), and every price is piecewise constant in the limit.
        let request = serde_json::json!({
            "formulation": "dcopf",
            "constraints": [{
                "id": "line 1-2",
                "terms": [{ "kind": "branch_flow", "element": 1, "coefficient": 1.0 }],
                "upper": 50.0
            }],
            "sensitivities": [
                { "operand": {"Dispatch":"Active"}, "parameter": "ConstraintLimit" },
                { "operand": {"Price":"Active"}, "parameter": "ConstraintLimit" },
                { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"} }
            ]
        });
        let out = solve_test_network_json(&linear_cost_case3_json(), &request.to_string())
            .expect("solve");
        let response: SolveResponse = serde_json::from_str(&out).unwrap();
        let dispatch = &response.sensitivities[0];
        assert_eq!(dispatch.units, "(MW)/MW");
        assert_eq!(
            dispatch.cols[0].element,
            crate::sens::ElementId::Constraint(0)
        );
        // dg1/dlimit = 3, dg3/dlimit = -3.
        assert!(
            (dispatch.values[0][0] - 3.0).abs() < 1e-5,
            "{:?}",
            dispatch.values
        );
        assert!(
            (dispatch.values[1][0] + 3.0).abs() < 1e-5,
            "{:?}",
            dispatch.values
        );
        for row in &response.sensitivities[1].values {
            assert!(row[0].abs() < 1e-5, "{row:?}");
        }
        assert_eq!(response.sensitivities[2].units, "(objective_unit/MW)/MW");
    }

    #[test]
    fn capabilities_list_constraint_term_kinds_for_dc_opf_only() {
        for caps in formulation_caps() {
            if caps.formulation == Problem::DcOpf {
                assert_eq!(
                    caps.constraints,
                    vec![
                        ConstraintTermKind::BranchFlow,
                        ConstraintTermKind::BusInjection,
                        ConstraintTermKind::Generator,
                    ]
                );
            } else {
                assert!(caps.constraints.is_empty(), "{:?}", caps.formulation);
            }
        }
        let v: Value = serde_json::from_str(&capabilities_json()).unwrap();
        let dc_opf = v
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["formulation"] == "dcopf")
            .unwrap();
        assert_eq!(
            dc_opf["constraints"],
            serde_json::json!(["branch_flow", "bus_injection", "generator"])
        );
    }
}
