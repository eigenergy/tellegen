//! The unified, object-safe sensitivity contract: one physical vocabulary, one
//! trait, one driver, shared by the DC OPF KKT, the AC power flow Newton system, and
//! the conic SOCWR KKT.
//!
//! A formulation exposes its solved, differentiable system as a [`Differentiable`].
//! The trait is object safe by construction — every method takes `&self`, no
//! associated types, no generic methods, concrete arguments and returns — so
//! `&dyn Differentiable` is a legal type and a third-party formulation can implement
//! it out of crate. It is not sealed.
//!
//! [`sensitivity`] is the single front door: it validates support, resolves the
//! parameter columns and the [`Mode::Auto`] direction, builds `K`, `dK/dp`, and the
//! operand selector once, runs the shared [`forward_adjoint`] + [`solve_refined`],
//! then rescales to served units and attaches identity metadata. The superlinear
//! work (the sparse LU, the back-solves, the refinement) stays inside the monomorphic
//! `solve_refined`; the trait object is touched only O(1) times per request, so the
//! `dyn` boundary costs nothing.

use faer::Mat;
use serde::{Deserialize, Serialize};

use super::{forward_adjoint, solve_refined, Mode};

/// Active or reactive power. The orthogonal P/Q sub-axis shared by prices,
/// dispatch, flows, and demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Power {
    Active,
    Reactive,
}

/// Which end of a branch a flow is measured at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum End {
    From,
    To,
}

/// How a voltage operand is represented: polar (`Magnitude`, `Angle`) for the AC
/// power flow, or the W-space lift (`Squared` = `|V|²`, `ProductReal` = `Re(Vi Vj*)`,
/// `ProductImag` = `Im(Vi Vj*)`) for the conic relaxation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum VoltageKind {
    Magnitude,
    Angle,
    Squared,
    ProductReal,
    ProductImag,
}

/// Which generation-cost coefficient: the quadratic `cq` or the linear `cl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CostTerm {
    Quadratic,
    Linear,
}

/// Conductance `g` or susceptance `b`, the real/imaginary parts of an admittance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GB {
    Conductance,
    Susceptance,
}

/// A min or max bound, e.g. the lower/upper voltage-magnitude limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Bound {
    Min,
    Max,
}

/// A transformer control: the tap ratio or the phase-shift angle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TapKind {
    Ratio,
    PhaseShift,
}

/// What a sensitivity is taken *of*, named for the physical power quantity rather
/// than a formulation's internal variable. The orthogonal sub-axes (active/reactive,
/// from/to, voltage representation) are visible to the type system; each formulation
/// maps a request to its own KKT rows or returns [`SensError::Unsupported`].
///
/// The dialects fold into one vocabulary: DC `Angle` is `Voltage(Angle)`, the conic
/// squared magnitude `w` is `Voltage(Squared)`, the nodal price is `Price(Active)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Operand {
    /// Nodal-balance dual (the LMP for active power, the reactive price for
    /// reactive), per bus.
    Price(Power),
    /// Generator dispatch `pg` / `qg`, per generator.
    Dispatch(Power),
    /// Branch active/reactive flow at the from/to end, per branch.
    Flow { power: Power, end: End },
    /// Bus voltage in the formulation's representation (`vm` / `va` / `w` / `wr` /
    /// `wi`), per bus.
    Voltage(VoltageKind),
}

/// What a sensitivity is taken *with respect to* — the parameter whose hand-derived
/// `dK/dp` column drives the solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Parameter {
    /// Bus active/reactive demand `pd` / `qd`, per bus.
    Demand(Power),
    /// Generation cost coefficient, per generator.
    Cost(CostTerm),
    /// DC thermal limit `fmax` / AC-conic apparent-power `|S|` limit, per branch.
    LineLimit,
    /// Branch series admittance `g` / `b`, per branch.
    SeriesAdmittance(GB),
    /// Bus shunt admittance `gs` / `bs`, per bus.
    ShuntAdmittance(GB),
    /// Voltage-magnitude bound right-hand side (the `w = |V|²` bound), per bus.
    VoltageBound(Bound),
    /// Generator output bound right-hand side (`pmax` / `pmin` / `qmax` / `qmin`),
    /// per generator.
    GenBound { power: Power, bound: Bound },
    /// Transformer tap ratio / phase shift, per branch.
    Transformer(TapKind),
    /// Branch switching state `sw` in `[0, 1]`, per branch.
    Switching,
}

/// The element family an [`Operand`] or [`Parameter`] ranges over, so the driver can
/// turn a dense index into a source [`ElementId`] without knowing the formulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Axis {
    Bus,
    Branch,
    Generator,
}

impl Operand {
    /// The element family this operand's rows index. The W-space voltage products
    /// `wr`/`wi` live on branches; the polar/squared voltages and the prices on buses.
    pub fn axis(self) -> Axis {
        match self {
            Operand::Voltage(VoltageKind::ProductReal | VoltageKind::ProductImag)
            | Operand::Flow { .. } => Axis::Branch,
            Operand::Price(_) | Operand::Voltage(_) => Axis::Bus,
            Operand::Dispatch(_) => Axis::Generator,
        }
    }
}

impl Parameter {
    /// The element family this parameter's columns index.
    pub fn axis(self) -> Axis {
        match self {
            Parameter::Demand(_) | Parameter::ShuntAdmittance(_) | Parameter::VoltageBound(_) => {
                Axis::Bus
            }
            Parameter::Cost(_) | Parameter::GenBound { .. } => Axis::Generator,
            Parameter::LineLimit
            | Parameter::SeriesAdmittance(_)
            | Parameter::Transformer(_)
            | Parameter::Switching => Axis::Branch,
        }
    }
}

/// A source-id reference for a dense row or column, so a [`SensitivityMatrix`]
/// self-describes and the api serializes it without re-deriving id maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ElementId {
    Bus(usize),
    Branch(usize),
    Generator(usize),
}

/// The operand selector `S`: each reported row is a linear functional of the KKT
/// solution `z`, the dense element index it reports (so metadata can name it), and the
/// reporting sign that maps the raw variable to the reported quantity (e.g. the conic
/// price is `-z` on the balance row, so its sign is `-1`).
///
/// Most operands read a single solution row (a unit functional) — use [`new`](Selector::new).
/// A derived operand (e.g. an AC branch flow, a function of the voltage state) reads a
/// weighted sum of rows — use [`linear`](Selector::linear).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Selector {
    /// One linear functional per reported row: `row o = sum_{(r, w)} w · z[r]`. A unit
    /// selector has `map[o] = [(r, 1.0)]`.
    pub map: Vec<Vec<(usize, f64)>>,
    /// Dense element index (bus/branch/generator) each row reports, aligned with
    /// `map`. Equals `0..map.len()` for a full-range operand; differs when the operand
    /// spans a subset (e.g. the AC free buses).
    pub elements: Vec<usize>,
    /// Reporting sign: `+1` when the variable is the reported quantity, `-1` when it
    /// is its negative (the price-dual flip).
    pub sign: f64,
}

impl Selector {
    /// A unit-row selector: reported row `i` reads solution row `rows[i]` with weight 1.
    /// The common case (DC/conic variables, AC voltages, prices).
    pub fn new(rows: Vec<usize>, elements: Vec<usize>, sign: f64) -> Self {
        let map = rows.into_iter().map(|r| vec![(r, 1.0)]).collect();
        Selector {
            map,
            elements,
            sign,
        }
    }

    /// A general linear-map selector: reported row `i` is `sum w · z[r]` over `map[i]`.
    /// For derived operands like an AC branch flow, whose gradient spans several state
    /// rows.
    pub fn linear(map: Vec<Vec<(usize, f64)>>, elements: Vec<usize>, sign: f64) -> Self {
        Selector {
            map,
            elements,
            sign,
        }
    }
}

/// The per-formulation regularization the one [`solve_refined`] needs: a Tikhonov
/// term `eps`, a refinement sweep count, and the residual tolerance factor. DC is
/// `{1e-10, 8, 1e-12}`, the converged AC Newton system `{0, 0, 0}` (nonsingular,
/// no regularization), the conic KKT `{1e-9, 12, 1e-13}`.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct SolveSpec {
    pub eps: f64,
    pub refine_iters: usize,
    pub tol_factor: f64,
}

impl SolveSpec {
    /// Bundle the Tikhonov term, refinement count, and tolerance factor.
    pub fn new(eps: f64, refine_iters: usize, tol_factor: f64) -> Self {
        SolveSpec {
            eps,
            refine_iters,
            tol_factor,
        }
    }
}

/// A solved formulation that exposes the pieces the shared sensitivity driver needs.
///
/// Object safe by construction — `&self`, no associated types, no generics, concrete
/// returns — so `&dyn Differentiable` is legal and third-party formulations can
/// implement it out of crate. Not sealed. The methods are called O(1) times per
/// request (build `K` once, `dK/dp` once, the selector once) plus O(rows + cols) for
/// metadata; the heavy linear algebra never crosses the trait object.
pub trait Differentiable {
    /// Stable lowercase formulation tag, for [`SensError::Unsupported`] and
    /// diagnostics.
    fn formulation(&self) -> &'static str;

    /// Dimension of the KKT / Newton system (the side of `K`).
    fn dim(&self) -> usize;

    /// The system Jacobian `K = dK/dz` as `(row, col, value)` triplets.
    fn jacobian(&self) -> Vec<(usize, usize, f64)>;

    /// How many indices `p` ranges over, or `None` if this formulation does not
    /// support it.
    fn parameter_len(&self, p: Parameter) -> Option<usize>;

    /// The parameter Jacobian `dK/dp` for the requested dense indices, packed as the
    /// dense `dim × idx.len()` forward right-hand side. These are the natural
    /// `+dK/dp` columns; the driver applies the leading minus of
    /// `dz/dp = -K⁻¹ dK/dp`.
    fn parameter_jacobian(&self, p: Parameter, idx: &[usize]) -> Result<Mat<f64>, SensError>;

    /// How many indices the operand `o` ranges over, or `None` if unsupported.
    fn operand_len(&self, o: Operand) -> Option<usize>;

    /// The operand selector: which rows it reads, the element each row names, and
    /// the reporting sign.
    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError>;

    /// The per-formulation regularization for the shared solve.
    fn solve_spec(&self) -> SolveSpec;

    /// Map a dense index on `axis` to its source [`ElementId`].
    fn element_id(&self, axis: Axis, index: usize) -> ElementId;

    /// Per-unit → served-unit scale for the `(operand, parameter)` cell, applied at
    /// the result. `1.0` leaves the engine's per-unit values untouched.
    fn unit_scale(&self, o: Operand, p: Parameter) -> f64;
}

/// Identity and quantity metadata for one row of a [`SensitivityMatrix`].
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct RowMeta {
    pub operand: Operand,
    pub element: ElementId,
    /// Dense index of the element this row reports.
    pub index: usize,
}

/// Identity and quantity metadata for one column of a [`SensitivityMatrix`].
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ColMeta {
    pub parameter: Parameter,
    pub element: ElementId,
    /// Dense index of the element this column varies.
    pub index: usize,
}

/// A self-describing sensitivity result: `values[r][c] = d(rows[r])/d(cols[c])`,
/// with row and column metadata naming each quantity and its source element, and the
/// served-unit label.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct SensitivityMatrix {
    /// `values[r][c] = d(operand_r)/d(parameter_c)`, in the units named by `units`. The
    /// engine produces per-unit values; the api edge rescales to served units.
    pub values: Vec<Vec<f64>>,
    pub rows: Vec<RowMeta>,
    pub cols: Vec<ColMeta>,
    pub units: String,
}

/// The error surface of the sensitivity path, replacing the bare `String` the engines
/// used. `Unsupported` is raised before any factorization.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum SensError {
    /// The formulation does not answer this `(operand, parameter)` request.
    Unsupported {
        formulation: &'static str,
        operand: Operand,
        parameter: Parameter,
    },
    /// A requested index is out of range for its axis.
    IndexOutOfRange {
        axis: Axis,
        index: usize,
        len: usize,
    },
    /// Assembling the system or a parameter column failed.
    Assembly(String),
    /// The sparse solve failed.
    Solve(String),
    /// The request itself was malformed.
    InvalidInput(String),
}

impl std::fmt::Display for SensError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensError::Unsupported {
                formulation,
                operand,
                parameter,
            } => write!(
                f,
                "{formulation} does not support d({operand:?})/d({parameter:?})"
            ),
            SensError::IndexOutOfRange { axis, index, len } => {
                write!(f, "{axis:?} index {index} out of range (len {len})")
            }
            SensError::Assembly(m) => write!(f, "sensitivity assembly failed: {m}"),
            SensError::Solve(m) => write!(f, "sensitivity solve failed: {m}"),
            SensError::InvalidInput(m) => write!(f, "invalid sensitivity request: {m}"),
        }
    }
}

impl std::error::Error for SensError {}

/// Resolve [`Mode::Auto`] to the cheaper direction: forward solves once per parameter
/// column, adjoint once per operand row, so pick forward when the parameter count is
/// at most the operand count.
pub(crate) fn auto_mode(nparam: usize, noperand: usize) -> Mode {
    if nparam <= noperand {
        Mode::Forward
    } else {
        Mode::Adjoint
    }
}

/// Per-unit → served-unit scale for an operand: the factor that converts the engine's
/// per-unit operand to its engineering unit. Prices divide by the base (`$/MWh =
/// per-unit/base`), power quantities multiply (`MW/MVAr = per-unit·base`), voltages are
/// already per-unit.
fn operand_served_scale(o: Operand, base: f64) -> f64 {
    match o {
        Operand::Price(_) => 1.0 / base,
        Operand::Dispatch(_) | Operand::Flow { .. } => base,
        Operand::Voltage(_) => 1.0,
    }
}

/// Per-unit → served-unit scale for a parameter. Power quantities are MW/MVAr; the
/// W-space voltage bound, the costs, and the admittances keep their per-unit convention.
fn parameter_served_scale(p: Parameter, base: f64) -> f64 {
    match p {
        Parameter::Demand(_) | Parameter::LineLimit | Parameter::GenBound { .. } => base,
        Parameter::VoltageBound(_)
        | Parameter::Cost(_)
        | Parameter::SeriesAdmittance(_)
        | Parameter::ShuntAdmittance(_)
        | Parameter::Transformer(_)
        | Parameter::Switching => 1.0,
    }
}

/// The served-unit rescale for a `(operand, parameter)` cell: `op_scale / par_scale`,
/// in factors of the system base power. The engine implements `unit_scale` with this;
/// the api edge multiplies by it. (e.g. `(Price, Demand) = (1/base)/base = 1/base²`,
/// the `($/MWh)/MW` LMP-vs-demand cell; `(Dispatch, Demand) = base/base = 1`.)
pub(crate) fn served_unit_scale(o: Operand, p: Parameter, base: f64) -> f64 {
    operand_served_scale(o, base) / parameter_served_scale(p, base)
}

/// Served-unit symbol for an operand (P-quantities in MW / $/MWh, Q in MVAr / $/MVArh).
fn operand_unit(o: Operand) -> &'static str {
    match o {
        Operand::Price(Power::Active) => "$/MWh",
        Operand::Price(Power::Reactive) => "$/MVArh",
        Operand::Dispatch(Power::Active)
        | Operand::Flow {
            power: Power::Active,
            ..
        } => "MW",
        Operand::Dispatch(Power::Reactive)
        | Operand::Flow {
            power: Power::Reactive,
            ..
        } => "MVAr",
        Operand::Voltage(VoltageKind::Magnitude) => "pu",
        Operand::Voltage(VoltageKind::Angle) => "rad",
        Operand::Voltage(_) => "pu^2",
    }
}

/// Served-unit symbol for a parameter.
fn parameter_unit(p: Parameter) -> &'static str {
    match p {
        Parameter::Demand(Power::Active)
        | Parameter::GenBound {
            power: Power::Active,
            ..
        } => "MW",
        Parameter::Demand(Power::Reactive)
        | Parameter::GenBound {
            power: Power::Reactive,
            ..
        } => "MVAr",
        Parameter::LineLimit => "MVA",
        Parameter::VoltageBound(_) => "pu^2",
        Parameter::Cost(CostTerm::Quadratic) => "$/MWh^2",
        Parameter::Cost(CostTerm::Linear) => "$/MWh",
        Parameter::SeriesAdmittance(_) | Parameter::ShuntAdmittance(_) => "pu",
        Parameter::Transformer(TapKind::Ratio) => "ratio",
        Parameter::Transformer(TapKind::PhaseShift) => "rad",
        Parameter::Switching => "1",
    }
}

/// The served-unit label `"(operand-unit)/parameter-unit"` for a cell, e.g.
/// `"($/MWh)/MW"`. The api stamps this when it rescales a result to served units.
pub(crate) fn served_units_label(o: Operand, p: Parameter) -> String {
    format!("({})/{}", operand_unit(o), parameter_unit(p))
}

/// `d(operand)/d(parameter)` for a solved formulation, the single object-safe front
/// door over DC, AC, and conic.
///
/// Validates support (raising [`SensError::Unsupported`] before any factorization),
/// resolves the parameter columns (`indices = None` means all), builds `K`, `dK/dp`,
/// and the operand selector once, resolves [`Mode::Auto`], runs the shared
/// [`forward_adjoint`] + [`solve_refined`], then rescales by `unit_scale` and attaches
/// identity metadata. Forward and adjoint return the same matrix; the leading minus of
/// `dz/dp = -K⁻¹ dK/dp` composes with the selector's reporting sign.
pub fn sensitivity(
    sys: &dyn Differentiable,
    operand: Operand,
    parameter: Parameter,
    indices: Option<&[usize]>,
    mode: Mode,
) -> Result<SensitivityMatrix, SensError> {
    let unsupported = || SensError::Unsupported {
        formulation: sys.formulation(),
        operand,
        parameter,
    };
    let op_len = sys.operand_len(operand).ok_or_else(unsupported)?;
    let par_len = sys.parameter_len(parameter).ok_or_else(unsupported)?;

    // Resolve and bounds-check the requested parameter columns.
    let cols: Vec<usize> = match indices {
        Some(ix) => {
            for &i in ix {
                if i >= par_len {
                    return Err(SensError::IndexOutOfRange {
                        axis: parameter.axis(),
                        index: i,
                        len: par_len,
                    });
                }
            }
            ix.to_vec()
        }
        None => (0..par_len).collect(),
    };

    let selector = sys.operand_selector(operand)?;
    // The operand's length must agree across its two trait methods: the values matrix
    // has one row per `selector.map` entry, while the row metadata below is built over
    // `operand_len` and indexes `selector.elements`. A divergence would return a matrix
    // whose rows and values disagree, or panic-index `elements`; surface it instead.
    if selector.map.len() != op_len || selector.elements.len() != op_len {
        return Err(SensError::Assembly(format!(
            "operand {operand:?}: operand_len {op_len} disagrees with selector (map {}, elements {})",
            selector.map.len(),
            selector.elements.len(),
        )));
    }
    let dim = sys.dim();
    let kkt = sys.jacobian();
    let dkdp = sys.parameter_jacobian(parameter, &cols)?;

    let direction = match mode {
        Mode::Auto => auto_mode(cols.len(), selector.map.len()),
        m => m,
    };

    // forward_rhs is the natural +dK/dp; the leading minus of dz/dp = -K⁻¹ dK/dp
    // rides in the sign, composed with the operand's reporting flip.
    let spec = sys.solve_spec();
    let sign = -selector.sign;
    let values = forward_adjoint(dim, &kkt, dkdp, &selector.map, sign, direction, |t, rhs| {
        solve_refined(dim, t, rhs, spec.eps, spec.refine_iters, spec.tol_factor)
    })
    .map_err(SensError::Solve)?;

    // The engine stays per-unit; the served-unit rescale (`unit_scale`) is applied at
    // the api edge, per the engine invariant.
    let rows = (0..op_len)
        .map(|o| {
            let index = selector.elements[o];
            RowMeta {
                operand,
                element: sys.element_id(operand.axis(), index),
                index,
            }
        })
        .collect();
    let cols_meta = cols
        .iter()
        .map(|&c| ColMeta {
            parameter,
            element: sys.element_id(parameter.axis(), c),
            index: c,
        })
        .collect();

    Ok(SensitivityMatrix {
        values,
        rows,
        cols: cols_meta,
        // Engine results stay per-unit; the api edge overwrites this with the served
        // label when it rescales (see `served_units_label`).
        units: "per unit".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `Differentiable` whose only job is to prove the trait is object
    /// safe: if `&dyn Differentiable` were not a legal type, this would not compile.
    struct Mock;

    impl Differentiable for Mock {
        fn formulation(&self) -> &'static str {
            "mock"
        }
        fn dim(&self) -> usize {
            0
        }
        fn jacobian(&self) -> Vec<(usize, usize, f64)> {
            Vec::new()
        }
        fn parameter_len(&self, _p: Parameter) -> Option<usize> {
            None
        }
        fn parameter_jacobian(&self, _p: Parameter, _idx: &[usize]) -> Result<Mat<f64>, SensError> {
            Ok(Mat::zeros(0, 0))
        }
        fn operand_len(&self, _o: Operand) -> Option<usize> {
            None
        }
        fn operand_selector(&self, _o: Operand) -> Result<Selector, SensError> {
            Ok(Selector::new(Vec::new(), Vec::new(), 1.0))
        }
        fn solve_spec(&self) -> SolveSpec {
            SolveSpec::new(0.0, 0, 0.0)
        }
        fn element_id(&self, _axis: Axis, index: usize) -> ElementId {
            ElementId::Bus(index)
        }
        fn unit_scale(&self, _o: Operand, _p: Parameter) -> f64 {
            1.0
        }
    }

    #[test]
    fn differentiable_is_object_safe() {
        let mock = Mock;
        let dynref: &dyn Differentiable = &mock;
        assert_eq!(dynref.dim(), 0);
        assert_eq!(dynref.formulation(), "mock");
        // An unsupported request on the dyn object surfaces Unsupported, not a panic.
        let err = sensitivity(
            dynref,
            Operand::Price(Power::Active),
            Parameter::Demand(Power::Active),
            None,
            Mode::Auto,
        )
        .unwrap_err();
        assert!(matches!(err, SensError::Unsupported { .. }));
    }

    #[test]
    fn auto_picks_the_smaller_dimension() {
        // Fewer (or equal) parameters than operands -> forward (one solve per param).
        assert_eq!(auto_mode(2, 3), Mode::Forward);
        assert_eq!(auto_mode(3, 3), Mode::Forward);
        // More parameters than operands -> adjoint (one solve per operand).
        assert_eq!(auto_mode(5, 3), Mode::Adjoint);
    }
}
