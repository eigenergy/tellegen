//! Sensitivities of the balanced AC OPF by implicit differentiation of its KKT
//! system with the active set held fixed.
//!
//! The solved program is the polar model of [`crate::model`]: variables
//! `(va, vm, pg, qg, cost epigraphs)`, nodal balance equalities, reference
//! angles, angle-difference and thermal inequalities, and variable bounds on
//! voltage magnitude and generation. At an accepted POUNCE point this module
//! keeps the equalities, the inequality rows at a bound and the variables at a
//! bound, and differentiates
//!
//! ```text
//!   grad f + J_A' lambda_A + I_B' nu_B = 0
//!   g_A(x, p) - bound_A(p)            = 0
//!   x_B - bound_B(p)                  = 0
//! ```
//!
//! in `z = (x, lambda_A, nu_B)`. The derivative evaluation, the matrix and its
//! qualification are the shared [`nlp_kkt`](super::nlp_kkt) code the
//! multiconductor OPF uses.
//!
//! The derivative exists only where the active set does not change under a
//! small parameter change. [`AcOpfKkt::new`] therefore refuses, with
//! [`SensError::Degenerate`], a point where the slack and the multiplier of a
//! bound are comparable (an inequality at its bound with a zero multiplier is
//! the limiting case), where the solver's multipliers do not satisfy
//! stationarity, or where the matrix is singular. The matrix is singular when
//! the active constraint gradients are linearly dependent, and also when the
//! solution is not unique: several generators at one bus can share its
//! reactive power in any proportion, and such a case is refused. The result
//! is conditional on the local solution POUNCE returned; it is not a
//! statement about other local optima.

use std::collections::BTreeMap;

use faer::Mat;

use super::interface::{
    Axis, Bound, Differentiable, ElementId, End, Operand, Parameter, Power, Selector, SensError,
    SolveSpec, VoltageKind,
};
use super::nlp_kkt::{
    check_stationarity, constraint_values, KktErrorKind, KktFactor, NlpDerivatives,
};
use crate::model::{AcOpfDualPoint, AcOpfSolved};

/// POUNCE relaxes every bound by this factor times `max(1, |bound|)` before it
/// solves, so a returned point may sit that far beyond the stated bound. The
/// slacks below are measured to the relaxed bound.
const BOUND_RELAXATION: f64 = 1e-8;
/// An active bound's slack, relative to `1 + |bound|`, is at most this.
const ACTIVE_TOL: f64 = 1e-4;
/// An inactive bound's multiplier, relative to `1 +` the largest multiplier,
/// is at most this, and its relative slack is above [`SLACK_FLOOR`].
const ZERO_MULTIPLIER_TOL: f64 = 1e-6;
const SLACK_FLOOR: f64 = 1e-7;
/// An active bound's relative multiplier is above this.
const MULTIPLIER_FLOOR: f64 = 1e-9;
/// Of a bound's relative slack and relative multiplier, the larger must exceed
/// the smaller by this factor for the bound to count as inactive or active.
const SEPARATION: f64 = 1e3;
/// Stationarity tolerance relative to `1 +` the size of the terms summed.
const KKT_TOL: f64 = 1e-6;

fn has_lower(value: f64) -> bool {
    value.is_finite() && value > -1e19
}

fn has_upper(value: f64) -> bool {
    value.is_finite() && value < 1e19
}

fn degenerate(message: impl Into<String>) -> SensError {
    SensError::Degenerate(message.into())
}

/// Source ids for the dense bus, branch and generator axes of the response.
#[derive(Clone, Debug)]
pub(crate) struct AcOpfElementIds {
    pub(crate) bus: Vec<usize>,
    pub(crate) branch: Vec<usize>,
    pub(crate) generator: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Lower,
    Upper,
    /// Lower and upper bound coincide.
    Fixed,
}

/// Where a quantity stands between one of its bounds and that bound's
/// multiplier.
enum Standing {
    Active,
    Inactive,
}

/// The slack of `value` to one side of `bound`, measured to the bound POUNCE
/// relaxed and relative to `1 + |bound|`. `gap` is the signed distance into
/// the feasible side of the stated bound.
fn relative_slack(gap: f64, bound: f64) -> f64 {
    (gap + BOUND_RELAXATION * bound.abs().max(1.)).max(0.) / (1. + bound.abs())
}

/// Classify one bound from its slack and the multiplier pushing against it,
/// both already relative to their scales and nonnegative. `what` names the
/// constraint for the error.
///
/// An interior-point solution keeps `slack * multiplier` near its final
/// barrier parameter. A bound that shapes the solution has a multiplier far
/// larger than its slack; one that does not has a slack far larger than its
/// multiplier; a degenerate one has the two comparable, and is refused.
fn standing(slack: f64, multiplier: f64, what: &dyn Fn() -> String) -> Result<Standing, SensError> {
    if slack <= ACTIVE_TOL && multiplier > MULTIPLIER_FLOOR && multiplier >= SEPARATION * slack {
        Ok(Standing::Active)
    } else if multiplier <= ZERO_MULTIPLIER_TOL
        && slack > SLACK_FLOOR
        && slack >= SEPARATION * multiplier
    {
        Ok(Standing::Inactive)
    } else {
        Err(degenerate(format!(
            "{} has a relative slack of {slack:.1e} to its bound and a multiplier of relative size {multiplier:.1e}; neither is clearly zero against the other, so the solver's point does not say whether the bound is active, and the active set may change under an arbitrarily small parameter change",
            what()
        )))
    }
}

/// The fixed-active-set KKT system of one accepted AC OPF point.
pub(crate) struct AcOpfKkt<'a> {
    solved: &'a AcOpfSolved,
    dual: &'a AcOpfDualPoint,
    ids: AcOpfElementIds,
    /// Number of program variables.
    n: usize,
    /// Compiled row -> position among the active rows.
    active: BTreeMap<usize, usize>,
    /// Variable column -> (position among the bound rows, which bound).
    bounds: BTreeMap<usize, (usize, Side)>,
    /// The qualified matrix; its entries are the Jacobian the driver solves.
    factor: KktFactor,
}

impl std::fmt::Debug for AcOpfKkt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcOpfKkt")
            .field("dimension", &self.factor.dimension())
            .field("active_rows", &self.active.len())
            .field("active_bounds", &self.bounds.len())
            .finish()
    }
}

impl<'a> AcOpfKkt<'a> {
    /// Determine the active set of `solved`, check it, and factor its KKT
    /// matrix once. No optimization is rerun.
    pub(crate) fn new(solved: &'a AcOpfSolved, ids: AcOpfElementIds) -> Result<Self, SensError> {
        let dual = solved.dual.as_ref().ok_or_else(|| {
            degenerate("the solver returned no multipliers for this AC OPF solution")
        })?;
        let prep = &solved.preparation;
        if ids.bus.len() != prep.n_buses
            || ids.branch.len() != prep.n_branches()
            || ids.generator.len() != prep.n_generators()
        {
            return Err(SensError::Assembly(
                "element ids do not match the AC OPF preparation".into(),
            ));
        }
        let p = &dual.problem;
        let (x, n) = (&dual.x, p.n);
        let values = constraint_values(p, x).map_err(|e| SensError::Assembly(e.message))?;
        let multiplier_scale = 1.
            + dual
                .lambda
                .iter()
                .chain(&dual.z_lower)
                .chain(&dual.z_upper)
                .fold(0.0_f64, |largest, value| largest.max(value.abs()));

        let mut active = BTreeMap::new();
        let mut lambda = vec![0.; p.m];
        for row in 0..p.m {
            let (lower, upper) = (p.g_l[row], p.g_u[row]);
            if has_lower(lower) && lower == upper {
                active.insert(row, active.len());
                lambda[row] = dual.lambda[row];
                continue;
            }
            let what = || row_name(dual, row);
            let multiplier = dual.lambda[row] / multiplier_scale;
            let mut row_active = false;
            // A row at its lower bound pushes with a negative multiplier, one
            // at its upper bound with a positive multiplier.
            for (bound, present, slack, pushing) in [
                (lower, has_lower(lower), values[row] - lower, -multiplier),
                (upper, has_upper(upper), upper - values[row], multiplier),
            ] {
                if !present {
                    continue;
                }
                let slack = relative_slack(slack, bound);
                // A multiplier of the wrong sign for this side does not hold
                // the row here; it counts as zero.
                if matches!(standing(slack, pushing.max(0.), &what)?, Standing::Active) {
                    if row_active {
                        return Err(degenerate(format!("{} is at both of its bounds", what())));
                    }
                    row_active = true;
                }
            }
            if row_active {
                active.insert(row, active.len());
                lambda[row] = dual.lambda[row];
            }
        }

        let mut bounds = BTreeMap::new();
        for (column, &value) in x.iter().enumerate() {
            let (lower, upper) = (p.x_l[column], p.x_u[column]);
            if has_lower(lower) && lower == upper {
                bounds.insert(column, (bounds.len(), Side::Fixed));
                continue;
            }
            let what = || column_name(dual, column);
            for (bound, present, slack, z, side) in [
                (
                    lower,
                    has_lower(lower),
                    value - lower,
                    dual.z_lower[column],
                    Side::Lower,
                ),
                (
                    upper,
                    has_upper(upper),
                    upper - value,
                    dual.z_upper[column],
                    Side::Upper,
                ),
            ] {
                if !present {
                    continue;
                }
                let slack = relative_slack(slack, bound);
                if matches!(
                    standing(slack, z.max(0.) / multiplier_scale, &what)?,
                    Standing::Active
                ) {
                    if bounds.contains_key(&column) {
                        return Err(degenerate(format!("{} is at both of its bounds", what())));
                    }
                    bounds.insert(column, (bounds.len(), side));
                }
            }
        }

        let derivatives =
            NlpDerivatives::evaluate(p, x, &lambda).map_err(|e| SensError::Assembly(e.message))?;
        // The solver's point must be stationary with every multiplier it
        // returned. The inactive ones are then dropped from the linearization;
        // each is far smaller than its slack by the classification above.
        let every_row: BTreeMap<usize, usize> = (0..p.m).map(|row| (row, row)).collect();
        let (mut residual, mut scale) = derivatives.stationarity(&dual.lambda, &every_row);
        for column in 0..n {
            residual[column] += dual.z_upper[column] - dual.z_lower[column];
            scale[column] += dual.z_upper[column].abs() + dual.z_lower[column].abs();
        }
        for (&column, &(_, side)) in &bounds {
            // An equality's multiplier is free in sign; it takes whatever
            // value balances the column.
            if side == Side::Fixed {
                residual[column] = 0.;
            }
        }
        check_stationarity(&residual, &scale, KKT_TOL).map_err(|_| {
            degenerate(
                "the solver's multipliers do not satisfy stationarity closely enough to differentiate the solution",
            )
        })?;

        let mut bound_columns = vec![0; bounds.len()];
        for (&column, &(position, _)) in &bounds {
            bound_columns[position] = column;
        }
        let dim = n + active.len() + bound_columns.len();
        let factor = KktFactor::new(dim, derivatives.kkt_entries(&active, &bound_columns))
            .map_err(|e| match e.kind {
                KktErrorKind::Singular => degenerate(format!(
                    "the KKT matrix of the active set is singular or too badly conditioned ({}); the active constraint gradients are linearly dependent or the reduced Hessian is singular",
                    e.message
                )),
                _ => SensError::Assembly(e.message),
            })?;
        Ok(Self {
            solved,
            dual,
            ids,
            n,
            active,
            bounds,
            factor,
        })
    }

    fn base(&self) -> f64 {
        self.solved.preparation.base_mva
    }

    /// KKT row of a compiled constraint row, when it is active.
    fn active_row(&self, row: usize) -> Option<usize> {
        self.active.get(&row).map(|&a| self.n + a)
    }

    /// KKT row of a variable's bound, when the named side holds it.
    fn bound_row(&self, column: usize, bound: Bound) -> Result<Option<usize>, SensError> {
        let Some(&(position, side)) = self.bounds.get(&column) else {
            return Ok(None);
        };
        let row = self.n + self.active.len() + position;
        match (side, bound) {
            (Side::Lower, Bound::Min) | (Side::Upper, Bound::Max) => Ok(Some(row)),
            (Side::Lower, Bound::Max) | (Side::Upper, Bound::Min) => Ok(None),
            (Side::Fixed, _) => Err(SensError::InvalidInput(format!(
                "{} has equal lower and upper bounds, so moving one of them alone has no two-sided derivative",
                column_name(self.dual, column)
            ))),
        }
    }

    /// Gradient of one terminal flow of `branch` in the program variables.
    fn flow_gradient(&self, branch: usize, power: Power, end: End) -> Vec<(usize, f64)> {
        let prep = &self.solved.preparation;
        let columns = &self.dual.columns;
        let (f, t) = (prep.branches.from_bus[branch], prep.branches.to_bus[branch]);
        let [yff, yft, ytf, ytt] = prep.branches.calc_branch_admittance(branch);
        // flow = a vf^2 + b vt^2 + c wr + d wi with wr + j wi = Vf conj(Vt),
        // the same expressions the model compiles.
        let (a, b, c, d) = match (power, end) {
            (Power::Active, End::From) => (yff.re, 0., yft.re, yft.im),
            (Power::Reactive, End::From) => (-yff.im, 0., -yft.im, yft.re),
            (Power::Active, End::To) => (0., ytt.re, ytf.re, -ytf.im),
            (Power::Reactive, End::To) => (0., -ytt.im, -ytf.im, -ytf.re),
        };
        let (vf, vt) = (self.solved.vm[f], self.solved.vm[t]);
        let angle = self.solved.va[f] - self.solved.va[t];
        let (sin, cos) = angle.sin_cos();
        let (wr, wi) = (vf * vt * cos, vf * vt * sin);
        let d_angle = -c * wi + d * wr;
        vec![
            (columns.va[f], d_angle),
            (columns.va[t], -d_angle),
            (columns.vm[f], 2. * a * vf + c * vt * cos + d * vt * sin),
            (columns.vm[t], 2. * b * vt + c * vf * cos + d * vf * sin),
        ]
    }
}

fn row_name(dual: &AcOpfDualPoint, row: usize) -> String {
    let rows = &dual.rows;
    let branch_of = |list: &[Option<usize>]| list.iter().position(|&r| r == Some(row));
    if let Some(branch) = branch_of(&rows.thermal_from) {
        format!("the from-end thermal limit of branch column {branch}")
    } else if let Some(branch) = branch_of(&rows.thermal_to) {
        format!("the to-end thermal limit of branch column {branch}")
    } else if let Some(branch) = branch_of(&rows.angle_difference) {
        format!("the angle-difference limit of branch column {branch}")
    } else if let Some(generator) = rows
        .piecewise_segments
        .iter()
        .position(|segments| segments.contains(&row))
    {
        format!("a piecewise cost segment of generator column {generator}")
    } else {
        format!("constraint row {row}")
    }
}

fn column_name(dual: &AcOpfDualPoint, column: usize) -> String {
    let columns = &dual.columns;
    let find = |list: &[usize]| list.iter().position(|&c| c == column);
    if let Some(bus) = find(&columns.vm) {
        format!("the voltage magnitude of bus column {bus}")
    } else if let Some(generator) = find(&columns.pg) {
        format!("the active power of generator column {generator}")
    } else if let Some(generator) = find(&columns.qg) {
        format!("the reactive power of generator column {generator}")
    } else if let Some(bus) = find(&columns.va) {
        format!("the voltage angle of bus column {bus}")
    } else {
        format!("variable {column}")
    }
}

impl Differentiable for AcOpfKkt<'_> {
    fn formulation(&self) -> &'static str {
        "acopf"
    }

    fn dim(&self) -> usize {
        self.factor.dimension()
    }

    fn jacobian(&self) -> Vec<(usize, usize, f64)> {
        self.factor.entries().to_vec()
    }

    fn parameter_len(&self, p: Parameter) -> Option<usize> {
        let prep = &self.solved.preparation;
        match p {
            Parameter::Demand(_) => Some(prep.n_buses),
            Parameter::LineLimit => Some(prep.n_branches()),
            Parameter::GenBound { .. } => Some(prep.n_generators()),
            _ => None,
        }
    }

    fn parameter_jacobian(&self, p: Parameter, idx: &[usize]) -> Result<Mat<f64>, SensError> {
        let prep = &self.solved.preparation;
        let rows = &self.dual.rows;
        let columns = &self.dual.columns;
        let mut j = Mat::<f64>::zeros(self.dim(), idx.len());
        for (c, &element) in idx.iter().enumerate() {
            match p {
                // The balance row is generation minus demand minus the power
                // the network takes: demand enters with -1.
                Parameter::Demand(power) => {
                    let row = match power {
                        Power::Active => rows.p_balance[element],
                        Power::Reactive => rows.q_balance[element],
                    };
                    let row = self.active_row(row).ok_or_else(|| {
                        SensError::Assembly("a balance row is missing from the active set".into())
                    })?;
                    j[(row, c)] = -1.;
                }
                // The thermal rows are `s_max^2 - |S|^2 >= 0`; an inactive
                // limit or an unlimited branch has no effect.
                Parameter::LineLimit => {
                    for row in [rows.thermal_from[element], rows.thermal_to[element]]
                        .into_iter()
                        .flatten()
                    {
                        if let Some(row) = self.active_row(row) {
                            j[(row, c)] = 2. * prep.branches.s_max[element];
                        }
                    }
                }
                // An active bound row is `x - bound = 0`.
                Parameter::GenBound { power, bound } => {
                    let column = match power {
                        Power::Active => columns.pg[element],
                        Power::Reactive => columns.qg[element],
                    };
                    if let Some(row) = self.bound_row(column, bound)? {
                        j[(row, c)] = -1.;
                    }
                }
                _ => {
                    return Err(SensError::Assembly(format!(
                        "acopf has no parameter column for {p:?}"
                    )))
                }
            }
        }
        Ok(j)
    }

    fn operand_len(&self, o: Operand) -> Option<usize> {
        let prep = &self.solved.preparation;
        match o {
            Operand::Price(_) => (prep.objective
                == powerio_matrix::PreparedObjective::NetworkGeneratorCost)
                .then_some(prep.n_buses),
            Operand::Voltage(VoltageKind::Magnitude | VoltageKind::Angle) => Some(prep.n_buses),
            Operand::Dispatch(_) => Some(prep.n_generators()),
            Operand::Flow { .. } => Some(prep.n_branches()),
            _ => None,
        }
    }

    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError> {
        let rows = &self.dual.rows;
        let columns = &self.dual.columns;
        let unit = |list: Vec<usize>, sign: f64| {
            let elements = (0..list.len()).collect();
            Selector::new(list, elements, sign)
        };
        let balance = |list: &[usize]| {
            list.iter()
                .map(|&row| {
                    self.active_row(row).ok_or_else(|| {
                        SensError::Assembly("a balance row is missing from the active set".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match o {
            // The price is minus the balance multiplier.
            Operand::Price(Power::Active) => unit(balance(&rows.p_balance)?, -1.),
            Operand::Price(Power::Reactive) => unit(balance(&rows.q_balance)?, -1.),
            Operand::Dispatch(Power::Active) => unit(columns.pg.clone(), 1.),
            Operand::Dispatch(Power::Reactive) => unit(columns.qg.clone(), 1.),
            Operand::Voltage(VoltageKind::Magnitude) => unit(columns.vm.clone(), 1.),
            Operand::Voltage(VoltageKind::Angle) => unit(columns.va.clone(), 1.),
            Operand::Flow { power, end } => {
                let m = self.solved.preparation.n_branches();
                Selector::linear(
                    (0..m)
                        .map(|branch| self.flow_gradient(branch, power, end))
                        .collect(),
                    (0..m).collect(),
                    1.,
                )
            }
            _ => {
                return Err(SensError::Assembly(format!(
                    "acopf has no operand rows for {o:?}"
                )))
            }
        })
    }

    fn solve_spec(&self) -> SolveSpec {
        // The matrix passed its singularity and conditioning check at
        // construction, so the driver solves it without a Tikhonov term and
        // refines against the same matrix.
        SolveSpec::new(0., 4, 1e-13)
    }

    fn element_id(&self, axis: Axis, index: usize) -> ElementId {
        match axis {
            Axis::Bus => ElementId::Bus(self.ids.bus[index]),
            Axis::Branch => ElementId::Branch(self.ids.branch[index]),
            Axis::Generator => ElementId::Generator(self.ids.generator[index]),
            Axis::Constraint => ElementId::Constraint(index),
        }
    }

    fn unit_scale(&self, o: Operand, p: Parameter) -> f64 {
        super::served_unit_scale(o, p, self.base())
    }
}

#[cfg(test)]
#[path = "acopf_tests.rs"]
mod tests;
