//! Compiled-coordinate multipliers and conditional envelope derivatives.
//! No KCL multiplier is interpreted as a nodal energy price.
use super::*;
use pounce_rs::SparsityRequest;

pub(super) const KKT_TOL: f64 = 1e-6;
pub(super) const ACTIVE_TOL: f64 = 1e-7;
const RANK_TOL: f64 = 1e-9;
const MAX_ACTIVE_ROWS: usize = 256;
const MAX_RANK_ENTRIES: usize = 262_144;
// Match NlProblem/POUNCE's directional unbounded-side sentinels.
pub(super) fn has_lower(v: f64) -> bool {
    v.is_finite() && v > -1e19
}
pub(super) fn has_upper(v: f64) -> bool {
    v.is_finite() && v < 1e19
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfConstraintId {
    /// Unique compiler ordinal, including omitted and synthetic rows. Labels
    /// alone need not be unique when user identities contain separators.
    pub ordinal: usize,
    pub label: String,
    pub synthetic: bool,
    /// If present, the row is a squared norm divided by this working-unit
    /// radius squared. Other rows use their compiled expression coordinates.
    pub norm_divisor_pu: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfRowOmission {
    FixedEquation,
    ZeroReduction,
}
pub(super) use self::McOpfRowOmission as Omission;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfOmittedConstraint {
    pub identity: McOpfConstraintId,
    pub reason: McOpfRowOmission,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfConstraintDual {
    pub identity: McOpfConstraintId,
    pub value: f64,
    /// None denotes an infinite bound, never a missing measurement.
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    /// L = f + lambda*g; negative at a lower side, positive at an upper side.
    /// Equality multipliers are free. Original objective scale, compiled row units.
    pub multiplier: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfParameterOwner {
    Load,
    Generator,
    Inverter,
    Transformer,
    ControlProfile,
    Branch,
    VoltageLimit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfParameterKind {
    LoadActivePower,
    LoadReactivePower,
    /// Per-coil inverter availability edit: vary p_max[axis] and set p_avail
    /// to the sum, including all affected control references. Unit: W.
    Availability,
    ActivePowerLower,
    ActivePowerUpper,
    ReactivePowerLower,
    ReactivePowerUpper,
    CurrentRating,
    ApparentPowerRating,
    VoltageLower,
    VoltageUpper,
    TapLower,
    TapUpper,
    VoltVarBreakpoint,
    VoltWattBreakpoint,
    VoltVarFraction,
    VoltWattFraction,
    PowerFactor,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfParameter {
    pub owner: McOpfParameterOwner,
    pub component: String,
    /// Coil index for load/generator and inverter power parameters; conductor
    /// index for branches and inverter current ratings (coil index for delta).
    /// Transformer rating axes index result.ports; tap and voltage limits have
    /// no axis. Transformer component identities remain the transformer name.
    /// Control-profile axes index canonical breakpoint/limit arrays; power
    /// factor has no axis. Its component is the shared profile name.
    pub axis: Option<usize>,
    pub kind: McOpfParameterKind,
}
impl McOpfParameter {
    /// Identify a physical parameter. This constructs a selector, not a claim
    /// that the parameter exists or is differentiable in a particular result;
    /// sensitivity queries validate the complete identity against that result.
    pub fn new(
        owner: McOpfParameterOwner,
        component: &str,
        axis: Option<usize>,
        kind: McOpfParameterKind,
    ) -> Self {
        Self {
            owner,
            component: component.into(),
            axis,
            kind,
        }
    }
    /// Physical unit implied by the parameter kind. Availability and regularity
    /// are result-dependent and are checked separately by sensitivity queries.
    pub fn unit(&self) -> McOpfParameterUnit {
        use McOpfParameterKind::*;
        match self.kind {
            LoadActivePower | Availability | ActivePowerLower | ActivePowerUpper => {
                McOpfParameterUnit::Watt
            }
            LoadReactivePower | ReactivePowerLower | ReactivePowerUpper => McOpfParameterUnit::Var,
            CurrentRating => McOpfParameterUnit::Ampere,
            ApparentPowerRating => McOpfParameterUnit::VoltAmpere,
            VoltageLower | VoltageUpper | VoltVarBreakpoint | VoltWattBreakpoint => {
                McOpfParameterUnit::Volt
            }
            TapLower | TapUpper | VoltVarFraction | VoltWattFraction | PowerFactor => {
                McOpfParameterUnit::Ratio
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfParameterUnit {
    Watt,
    Var,
    Ampere,
    VoltAmpere,
    Volt,
    Ratio,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfMarginalStatus {
    /// First-order envelope value on the same regular local branch and active
    /// set. No second-order or global optimality/differentiability certificate.
    Conditional,
    FailedKktCheck,
    ZeroLimitReduction,
    RankDeficient,
    RankNotChecked,
    WeakActiveSet,
    UnsupportedParameter,
    CoincidentBounds,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfMarginalValue {
    pub parameter: McOpfParameter,
    pub parameter_unit: McOpfParameterUnit,
    /// d(objective in currency/hour)/d(parameter in the stated SI unit).
    /// Positive means increasing the parameter increases local operating cost.
    pub derivative: Option<f64>,
    pub status: McOpfMarginalStatus,
    pub row_ordinals: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfKktResiduals {
    pub stationarity_inf: f64,
    pub stationarity_relative_inf: f64,
    pub complementarity_inf: f64,
    pub complementarity_relative_inf: f64,
    pub dual_feasibility_inf: f64,
    pub primal_relative_inf: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfActiveRank {
    pub active_rows: usize,
    /// None when qualification could not finish within its resource budget.
    pub rank: Option<usize>,
    pub weak_inequalities: usize,
    /// Absent only in legacy version 2–6 reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification: Option<super::McOpfRankDiagnostics>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfDualReport {
    pub variables: Vec<String>,
    /// Working coordinates allow the stored diagnostics to be checked without
    /// rerunning optimization. They are not a public warm-start schema.
    pub primal: Vec<f64>,
    pub lower_bound_multipliers: Vec<f64>,
    pub upper_bound_multipliers: Vec<f64>,
    pub constraints: Vec<McOpfConstraintDual>,
    pub omitted: Vec<McOpfOmittedConstraint>,
    pub kkt: McOpfKktResiduals,
    pub active_rank: McOpfActiveRank,
    pub status: McOpfMarginalStatus,
    pub marginals: Vec<McOpfMarginalValue>,
}

#[derive(Clone, Debug)]
pub(super) enum Rule {
    Zero,
    Equality { scale: f64 },
    Bound { scale: f64, lower: bool },
    Norm { physical: f64 },
    Load(super::load_parameters::LoadParameter),
    Inverter(super::inverter_parameters::DroopParameter),
    Control(super::control_parameters::ControlParameter),
}
#[derive(Clone, Debug)]
pub(super) struct OmittedRow {
    pub origin: usize,
    pub reason: Omission,
}
#[derive(Clone, Debug, Default)]
pub(super) struct CompilerMap {
    pub origins: Vec<McOpfConstraintId>,
    pub rows: Vec<usize>,
    pub omitted: Vec<OmittedRow>,
    pub zero_limit: bool,
    pub parameters: BTreeMap<McOpfParameter, Vec<(Option<usize>, Rule)>>,
}
impl CompilerMap {
    pub fn origin(&mut self, label: String) -> usize {
        let ordinal = self.origins.len();
        self.origins.push(McOpfConstraintId {
            ordinal,
            label,
            synthetic: false,
            norm_divisor_pu: None,
        });
        ordinal
    }
    pub fn ensure(&mut self, parameter: McOpfParameter) {
        self.parameters.entry(parameter).or_default();
    }
    pub fn term(&mut self, parameter: McOpfParameter, origin: Option<usize>, rule: Rule) {
        self.parameters
            .entry(parameter)
            .or_default()
            .push((origin, rule));
    }
}

/// Exact compiled derivatives, original objective scale. POUNCE already removes
/// obj_scaling_factor in finalize_solution; dividing these duals again is wrong.
pub(super) fn analyze(
    model: &model::Model,
    x: &[f64],
    lambda: &[f64],
    zl: &[f64],
    zu: &[f64],
) -> Result<McOpfDualReport, String> {
    analyze_inner(model, x, lambda, zl, zu, 9)
}
#[cfg(test)]
pub(super) fn analyze_legacy(
    model: &model::Model,
    x: &[f64],
    lambda: &[f64],
    zl: &[f64],
    zu: &[f64],
) -> Result<McOpfDualReport, String> {
    analyze_inner(model, x, lambda, zl, zu, 6)
}
pub(super) fn analyze_inner(
    model: &model::Model,
    x: &[f64],
    lambda: &[f64],
    zl: &[f64],
    zu: &[f64],
    report_version: u32,
) -> Result<McOpfDualReport, String> {
    let p = &model.problem;
    if x.len() != p.n
        || lambda.len() != p.m
        || zl.len() != p.n
        || zu.len() != p.n
        || x.iter()
            .chain(lambda)
            .chain(zl)
            .chain(zu)
            .any(|v| !v.is_finite())
    {
        return Err("missing, malformed or nonfinite primal/multiplier arrays".into());
    }
    let mut t = NlTnlp::try_new(p.clone())?;
    let info = t.get_nlp_info().ok_or("missing derivative shape")?;
    let mut g = vec![0.; p.m];
    let mut grad = vec![0.; p.n];
    let mut ir = vec![0; info.nnz_jac_g as usize];
    let mut jc = ir.clone();
    let mut jac = vec![0.; ir.len()];
    if !t.eval_g(x, true, &mut g)
        || !t.eval_grad_f(x, true, &mut grad)
        || !t.eval_jac_g(
            None,
            false,
            SparsityRequest::Structure {
                irow: &mut ir,
                jcol: &mut jc,
            },
        )
        || !t.eval_jac_g(Some(x), true, SparsityRequest::Values { values: &mut jac })
    {
        return Err("compiled derivative evaluation failed".into());
    }
    let objective = t.eval_f(x, true).ok_or("objective evaluation failed")?;
    if g.iter()
        .chain(&grad)
        .chain(&jac)
        .chain(std::iter::once(&objective))
        .any(|v| !v.is_finite())
    {
        return Err("nonfinite compiled derivative evaluation".into());
    }
    let mut stationarity = grad.clone();
    let mut scales: Vec<_> = grad.iter().map(|v| 1. + v.abs()).collect();
    let mut row_norms = vec![0.0_f64; p.m];
    for ((&r, &c), &v) in ir.iter().zip(&jc).zip(&jac) {
        let (r, c) = (r as usize, c as usize);
        stationarity[c] += v * lambda[r];
        scales[c] += (v * lambda[r]).abs();
        row_norms[r] = row_norms[r].hypot(v);
    }
    let mut active = Vec::new();
    let mut weak = 0;
    let mut comp = 0.0_f64;
    let mut dual = 0.0_f64;
    let mut primal = 0.0_f64;
    for r in 0..p.m {
        let (lo, hi, value, multiplier) = (p.g_l[r], p.g_u[r], g[r], lambda[r]);
        let equality = lo == hi;
        let lower_active = has_lower(lo) && (value - lo).abs() <= ACTIVE_TOL * (1. + lo.abs());
        let upper_active = has_upper(hi) && (hi - value).abs() <= ACTIVE_TOL * (1. + hi.abs());
        if equality || lower_active || upper_active {
            active.push(r);
            if !equality && multiplier.abs() * row_norms[r] <= 1e-8 {
                weak += 1;
            }
        }
        if has_lower(lo) {
            primal = primal.max((lo - value).max(0.) / (1. + lo.abs()));
        }
        if has_upper(hi) {
            primal = primal.max((value - hi).max(0.) / (1. + hi.abs()));
        }
        if !equality {
            if multiplier < 0. {
                if has_lower(lo) {
                    comp = comp.max((multiplier * (value - lo)).abs());
                } else {
                    dual = dual.max(-multiplier);
                }
            } else if has_upper(hi) {
                comp = comp.max((multiplier * (hi - value)).abs());
            } else {
                dual = dual.max(multiplier);
            }
        }
    }
    // Current IVR compilation puts physical limits in rows. Keep bound terms
    // explicit so future finite variable bounds cannot silently corrupt KKT.
    let mut active_bounds = Vec::new();
    for i in 0..p.n {
        stationarity[i] += zu[i] - zl[i];
        scales[i] += zl[i].abs() + zu[i].abs();
        for (bound, z, lower) in [(p.x_l[i], zl[i], true), (p.x_u[i], zu[i], false)] {
            dual = dual.max((-z).max(0.));
            if !(if lower {
                has_lower(bound)
            } else {
                has_upper(bound)
            }) {
                dual = dual.max(z.abs());
                continue;
            }
            let slack = if lower { x[i] - bound } else { bound - x[i] };
            comp = comp.max((z * slack).abs());
            primal = primal.max((-slack).max(0.) / (1. + bound.abs()));
            if slack.abs() <= ACTIVE_TOL * (1. + bound.abs()) {
                active_bounds.push(i);
                if z <= 1e-8 {
                    weak += 1;
                }
            }
        }
    }
    let inf = stationarity.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if scales.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite stationarity normalization".into());
    }
    let relative = stationarity
        .iter()
        .zip(scales)
        .map(|(v, s)| v.abs() / s)
        .fold(0.0, f64::max);
    let kkt = McOpfKktResiduals {
        stationarity_inf: inf,
        stationarity_relative_inf: relative,
        complementarity_inf: comp,
        complementarity_relative_inf: comp / (1. + objective.abs()),
        dual_feasibility_inf: dual,
        primal_relative_inf: primal,
    };
    if stationarity.iter().any(|v| !v.is_finite())
        || [inf, relative, comp, dual, primal]
            .iter()
            .any(|v| !v.is_finite())
    {
        return Err("nonfinite KKT residual computation".into());
    }
    let count = active.len() + active_bounds.len();
    let (rank, qualification) = if report_version >= 7 {
        let lookup: BTreeMap<_, _> = active.iter().enumerate().map(|(i, &r)| (r, i)).collect();
        let mut rows = vec![BTreeMap::new(); count];
        for ((&r, &c), &v) in ir.iter().zip(&jc).zip(&jac) {
            if let Some(&i) = lookup.get(&(r as usize)) {
                *rows[i].entry(c as usize).or_default() += v;
            }
        }
        for (i, &c) in active_bounds.iter().enumerate() {
            rows[active.len() + i].insert(c, 1.);
        }
        for row in &mut rows {
            row.retain(|_, v| *v != 0.);
        }
        let (rank, audit) = if report_version <= 8 {
            super::rank::audit_legacy(rows, p.n)
        } else {
            super::rank::audit(rows, p.n)
        };
        (rank, Some(audit))
    } else if count > MAX_ACTIVE_ROWS || count.saturating_mul(p.n) > MAX_RANK_ENTRIES {
        (None, None)
    } else {
        let lookup: BTreeMap<_, _> = active.iter().enumerate().map(|(i, &r)| (r, i)).collect();
        let mut rows = vec![vec![0.; p.n]; count];
        for ((&r, &c), &v) in ir.iter().zip(&jc).zip(&jac) {
            if let Some(&i) = lookup.get(&(r as usize)) {
                rows[i][c as usize] += v;
            }
        }
        for (i, &c) in active_bounds.iter().enumerate() {
            rows[active.len() + i][c] = 1.;
        }
        (Some(row_rank(rows)), None)
    };
    let status = if [
        kkt.stationarity_relative_inf,
        kkt.complementarity_relative_inf,
        dual,
        primal,
    ]
    .iter()
    .any(|v| *v > KKT_TOL)
    {
        McOpfMarginalStatus::FailedKktCheck
    } else if model.dual_map.zero_limit {
        McOpfMarginalStatus::ZeroLimitReduction
    } else if rank.is_none() {
        McOpfMarginalStatus::RankNotChecked
    } else if rank != Some(count) {
        McOpfMarginalStatus::RankDeficient
    } else if weak > 0 {
        McOpfMarginalStatus::WeakActiveSet
    } else {
        McOpfMarginalStatus::Conditional
    };
    let constraints: Vec<_> = model
        .dual_map
        .rows
        .iter()
        .enumerate()
        .map(|(r, &id)| McOpfConstraintDual {
            identity: model.dual_map.origins[id].clone(),
            value: g[r],
            lower: has_lower(p.g_l[r]).then_some(p.g_l[r]),
            upper: has_upper(p.g_u[r]).then_some(p.g_u[r]),
            multiplier: lambda[r],
        })
        .collect();
    let lookup: BTreeMap<_, _> = model
        .dual_map
        .rows
        .iter()
        .enumerate()
        .map(|(r, &id)| (id, r))
        .collect();
    let marginals = model
        .dual_map
        .parameters
        .iter()
        .map(|(parameter, terms)| {
            let mut entry_status = status;
            let mut derivative = 0.;
            if status == McOpfMarginalStatus::Conditional {
                if terms.is_empty() {
                    entry_status = McOpfMarginalStatus::UnsupportedParameter;
                }
                for (origin, rule) in terms {
                    if matches!(rule, Rule::Zero) {
                        continue;
                    }
                    let Some(&r) = origin.and_then(|id| lookup.get(&id)) else {
                        entry_status = McOpfMarginalStatus::UnsupportedParameter;
                        continue;
                    };
                    derivative += match rule {
                        Rule::Zero => 0.,
                        Rule::Equality { scale } => -lambda[r] * scale,
                        Rule::Bound { scale, lower } => {
                            if p.g_l[r] == p.g_u[r] {
                                entry_status = McOpfMarginalStatus::CoincidentBounds;
                            }
                            if *lower {
                                (-lambda[r]).max(0.) * scale
                            } else {
                                -lambda[r].max(0.) * scale
                            }
                        }
                        Rule::Norm { physical } => -2. * lambda[r] * g[r] / physical,
                        Rule::Load(load) => match load.evaluate(x) {
                            Ok((value, _)) => lambda[r] * value,
                            Err(_) => {
                                entry_status = McOpfMarginalStatus::UnsupportedParameter;
                                0.
                            }
                        },
                        Rule::Inverter(control) => match control.evaluate(x) {
                            Ok((value, _)) => lambda[r] * value,
                            Err(_) => {
                                entry_status = McOpfMarginalStatus::UnsupportedParameter;
                                0.
                            }
                        },
                        Rule::Control(control) => match control.evaluate(x) {
                            Ok((value, _)) => lambda[r] * value,
                            Err(_) => {
                                entry_status = McOpfMarginalStatus::UnsupportedParameter;
                                0.
                            }
                        },
                    };
                }
            }
            McOpfMarginalValue {
                parameter: parameter.clone(),
                parameter_unit: parameter.unit(),
                derivative: (entry_status == McOpfMarginalStatus::Conditional
                    && derivative.is_finite())
                .then_some(derivative),
                status: if !derivative.is_finite() {
                    McOpfMarginalStatus::FailedKktCheck
                } else {
                    entry_status
                },
                row_ordinals: terms.iter().filter_map(|(id, _)| *id).collect(),
            }
        })
        .collect();
    Ok(McOpfDualReport {
        variables: p.var_names.clone(),
        primal: x.to_vec(),
        lower_bound_multipliers: zl.to_vec(),
        upper_bound_multipliers: zu.to_vec(),
        constraints,
        omitted: model
            .dual_map
            .omitted
            .iter()
            .map(|o| McOpfOmittedConstraint {
                identity: model.dual_map.origins[o.origin].clone(),
                reason: o.reason,
            })
            .collect(),
        kkt,
        active_rank: McOpfActiveRank {
            active_rows: count,
            rank,
            weak_inequalities: weak,
            qualification,
        },
        status,
        marginals,
    })
}

/// Pivoted, twice-reorthogonalized row QR after unit row normalization. This is
/// a bounded numerical LICQ audit, not a proof of uniqueness or second order sufficiency.
pub(super) fn row_rank(mut rows: Vec<Vec<f64>>) -> usize {
    for row in &mut rows {
        let norm = row.iter().fold(0.0_f64, |n, v| n.hypot(*v));
        if norm > 0. {
            for v in row {
                *v /= norm;
            }
        }
    }
    let mut rank = 0;
    while rank < rows.len() {
        let (pivot, norm) = (rank..rows.len())
            .map(|i| (i, rows[i].iter().map(|v| v * v).sum::<f64>().sqrt()))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        if norm <= RANK_TOL {
            break;
        }
        rows.swap(rank, pivot);
        for v in &mut rows[rank] {
            *v /= norm;
        }
        let (basis, rest) = rows.split_at_mut(rank + 1);
        for row in rest {
            for _ in 0..2 {
                let dot: f64 = row.iter().zip(&basis[rank]).map(|(a, b)| a * b).sum();
                for (v, q) in row.iter_mut().zip(&basis[rank]) {
                    *v -= dot * q;
                }
            }
        }
        rank += 1;
    }
    rank
}

impl McOpfDualReport {
    pub(super) fn verify(
        &self,
        model: &model::Model,
        solution: &McAcOpfSolution,
        tolerance: f64,
        report_version: u32,
    ) -> Result<validate::Checked, String> {
        if self.active_rank.qualification.as_ref().is_some_and(|q| {
            q.tolerance != super::rank::TOLERANCE
                || q.smallest_accepted_pivot.is_some_and(|v| !v.is_finite())
        }) {
            return Err("invalid rank qualification quantities".into());
        }
        if self.constraints.iter().any(|r| {
            [r.lower, r.upper, r.identity.norm_divisor_pu]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite())
        }) || self
            .omitted
            .iter()
            .any(|r| r.identity.norm_divisor_pu.is_some_and(|v| !v.is_finite()))
            || self
                .marginals
                .iter()
                .any(|m| m.derivative.is_some_and(|v| !v.is_finite()))
        {
            return Err("nonfinite optional dual quantity".into());
        }
        let lambda: Vec<_> = self.constraints.iter().map(|r| r.multiplier).collect();
        let recomputed = analyze_inner(
            model,
            &self.primal,
            &lambda,
            &self.lower_bound_multipliers,
            &self.upper_bound_multipliers,
            report_version,
        )?;
        let a = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let b = serde_json::to_value(&recomputed).map_err(|e| e.to_string())?;
        if !same_json(&a, &b) {
            return Err("inconsistent dual identities, residuals, rank or marginal values".into());
        }
        let checked = validate::check(model, &self.primal, solution.objective(), tolerance)
            .map_err(|e| e.to_string())?;
        validate::check_power_factors(
            solution.instance().network(),
            &checked.devices,
            model.prep.bases.power_base_va,
            tolerance,
        )
        .map_err(|e| e.to_string())?;
        for (terminal, pair) in model.prep.terminals.iter().zip(&model.voltage) {
            let re = pair[0].value(&self.primal) * model.prep.bases.voltage_base_v;
            let im = pair[1].value(&self.primal) * model.prep.bases.voltage_base_v;
            let magnitude = solution
                .terminal_voltage_magnitude(&terminal.bus, &terminal.terminal)
                .ok_or("missing canonical voltage")?;
            let angle = solution
                .terminal_voltage_angle(&terminal.bus, &terminal.terminal)
                .ok_or("missing canonical angle")?;
            if (re - magnitude * angle.cos()).hypot(im - magnitude * angle.sin())
                > 1e-7 * (1. + magnitude)
            {
                return Err("dual primal disagrees with canonical voltages".into());
            }
        }
        Ok(checked)
    }
}
pub(super) fn same_json(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            if a.is_f64() || b.is_f64() {
                match (a.as_f64(), b.as_f64()) {
                    (Some(a), Some(b)) => {
                        a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-12 + 1e-8 * b.abs()
                    }
                    _ => false,
                }
            } else {
                a == b
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_json(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, a)| b.get(k).is_some_and(|b| same_json(a, b)))
        }
        _ => a == b,
    }
}
