//! Fixed-active-set differentiation of a verified accepted stationary point.
use super::*;
use pounce_rs::SparsityRequest;
mod linear;
mod outputs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum McOpfSensitivityErrorKind {
    MissingDuals,
    StaleResult,
    InvalidResult,
    UnreliableDuals,
    UnsupportedParameter,
    UnsupportedOutput,
    InvalidRequest,
    DerivativeEvaluation,
    SingularKkt,
    LinearSolveFailure,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfSensitivityError {
    pub kind: McOpfSensitivityErrorKind,
    pub message: String,
    pub dual_status: Option<McOpfMarginalStatus>,
}
impl std::fmt::Display for McOpfSensitivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for McOpfSensitivityError {}
type Result<T> = std::result::Result<T, McOpfSensitivityError>;
use McOpfSensitivityErrorKind as ErrorKind;
fn error(kind: ErrorKind, message: impl Into<String>) -> McOpfSensitivityError {
    McOpfSensitivityError {
        kind,
        message: message.into(),
        dual_status: None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfVoltagePart {
    Real,
    Imaginary,
    Magnitude,
    Angle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfBranchEnd {
    From,
    To,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfFlowQuantity {
    CurrentReal,
    CurrentImaginary,
    ActivePower,
    ReactivePower,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "axis", rename_all = "snake_case", deny_unknown_fields)]
pub enum McOpfDeviceAxis {
    Coil {
        index: usize,
    },
    /// Local terminal label on the named device, including neutral returns.
    Terminal {
        terminal: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "axis", rename_all = "snake_case", deny_unknown_fields)]
pub enum McOpfTransformerAxis {
    Coil {
        index: usize,
    },
    /// Sum of bare winding-current incidence at this bus/terminal, including
    /// neutral returns. Separate core/grounding shunt ledgers are excluded.
    Terminal {
        terminal: McOpfTerminalId,
    },
    /// Prepared rating measurement, which may include exciting current or use
    /// a distinct through-power convention. Not an additive KCL injection.
    Port {
        name: String,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "output", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum McOpfSensitivityOutput {
    Objective,
    Voltage {
        terminal: McOpfTerminalId,
        part: McOpfVoltagePart,
    },
    /// Uses the physical result's load-withdrawal / source-injection convention.
    Device {
        kind: powerio_matrix::McOpfDeviceKind,
        identity: String,
        axis: McOpfDeviceAxis,
        quantity: McOpfFlowQuantity,
    },
    /// Current leaves the bus into the branch, including the selected end shunt.
    Branch {
        identity: String,
        end: McOpfBranchEnd,
        conductor: usize,
        quantity: McOpfFlowQuantity,
    },
    /// Currents enter the transformer, following the physical result ledgers.
    Transformer {
        identity: String,
        axis: McOpfTransformerAxis,
        quantity: McOpfFlowQuantity,
    },
    /// Optimized physical continuous tap ratio, not the internal multiplier.
    TransformerTap {
        identity: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfOutputUnit {
    CurrencyPerHour,
    Volt,
    Radian,
    Ampere,
    Watt,
    Var,
    Ratio,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfParameterDirection {
    pub parameter: McOpfParameter,
    /// d(parameter)/d(alpha), in the parameter's SI unit. Repeated entries add.
    pub delta: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfOutputWeight {
    pub output: McOpfSensitivityOutput,
    /// Coefficient in a scalar weighted sum of physical outputs. Callers own
    /// the units of this scalar when mixing quantities.
    pub weight: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfOutputDerivative {
    pub output: McOpfSensitivityOutput,
    pub unit: McOpfOutputUnit,
    /// d(output)/d(alpha) for the requested physical parameter direction.
    pub derivative: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfParameterDerivative {
    pub parameter: McOpfParameter,
    pub parameter_unit: McOpfParameterUnit,
    /// d(weighted output sum)/d(parameter in the stated SI unit).
    pub derivative: f64,
}

/// Reusable sparse factorization for one immutable, accepted result. Every
/// derivative is conditional on the same regular stationary branch and active
/// set, not a global optimum or second-order sufficiency certificate.
///
/// Borrowing a session's current result prevents edits while this view is alive.
/// An explicitly cloned historical result remains its own independent snapshot.
pub struct McOpfSensitivities<'a> {
    result: &'a McOpfResult,
    model: model::Model,
    factor: linear::Factor,
    active: BTreeMap<usize, usize>, // compiled row -> KKT constraint index
    jacobian: Vec<Vec<(usize, f64)>>,
    objective_gradient: Vec<f64>,
}
impl std::fmt::Debug for McOpfSensitivities<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McOpfSensitivities")
            .field("dimension", &self.factor.dimension())
            .field("active_rows", &self.active.len())
            .finish()
    }
}
impl McOpfResult {
    /// Requires `collect_duals`; verifies the result and factors its exact
    /// unregularized active KKT matrix once. No optimization is rerun.
    pub fn sensitivities(&self) -> Result<McOpfSensitivities<'_>> {
        if self.duals.is_none() {
            return Err(error(
                ErrorKind::MissingDuals,
                "solve with collect_duals enabled",
            ));
        }
        let model = self
            .verified_dual_model()
            .map_err(|e| error(ErrorKind::InvalidResult, e.to_string()))?
            .ok_or_else(|| error(ErrorKind::MissingDuals, "missing verified dual point"))?;
        McOpfSensitivities::new(self, model)
    }
}
impl McOpfSession {
    /// Only the current successful attempt is eligible; historical data must
    /// be selected explicitly through `last_accepted` instead.
    ///
    /// ```compile_fail,E0502
    /// use tellegen::{McOpfSession, McOpfSensitivityError};
    /// fn edit_while_borrowed(session: &mut McOpfSession) -> Result<(), McOpfSensitivityError> {
    ///     let view = session.sensitivities()?;
    ///     session.apply_edits(&[]).unwrap();
    ///     let _ = view.parameters();
    ///     Ok(())
    /// }
    /// ```
    pub fn sensitivities(&self) -> Result<McOpfSensitivities<'_>> {
        self.current_result()
            .ok_or_else(|| {
                error(
                    ErrorKind::StaleResult,
                    "session has no accepted result for its current revision and attempt",
                )
            })?
            .result
            .sensitivities()
    }
}

impl<'a> McOpfSensitivities<'a> {
    fn new(result: &'a McOpfResult, model: model::Model) -> Result<Self> {
        let d = result.duals.as_ref().expect("verified dual point");
        if d.status != McOpfMarginalStatus::Conditional {
            return Err(McOpfSensitivityError {
                kind: ErrorKind::UnreliableDuals,
                message: "a regular, strongly active dual point is required".into(),
                dual_status: Some(d.status),
            });
        }
        let p = &model.problem;
        let mut active = BTreeMap::new();
        let mut lambda = vec![0.; p.m];
        for (r, row) in d.constraints.iter().enumerate() {
            let at = |bound: Option<f64>| {
                bound.is_some_and(|b| (row.value - b).abs() <= duals::ACTIVE_TOL * (1. + b.abs()))
            };
            if row.lower != row.upper && at(row.lower) && at(row.upper) {
                return Err(McOpfSensitivityError {
                    kind: ErrorKind::UnreliableDuals,
                    message: "both sides of a narrow inequality are numerically active".into(),
                    dual_status: Some(McOpfMarginalStatus::WeakActiveSet),
                });
            }
            if (row.lower.is_some() && row.lower == row.upper) || at(row.lower) || at(row.upper) {
                active.insert(r, active.len());
                lambda[r] = row.multiplier;
            }
        }
        let mut bound_columns = Vec::new();
        for i in 0..p.n {
            for (bound, finite) in [
                (p.x_l[i], duals::has_lower(p.x_l[i])),
                (p.x_u[i], duals::has_upper(p.x_u[i])),
            ] {
                if finite && (d.primal[i] - bound).abs() <= duals::ACTIVE_TOL * (1. + bound.abs()) {
                    bound_columns.push(i);
                }
            }
        }
        let dim = p.n + active.len() + bound_columns.len();
        let mut t =
            NlTnlp::try_new(p.clone()).map_err(|e| error(ErrorKind::DerivativeEvaluation, e))?;
        let info = t
            .get_nlp_info()
            .ok_or_else(|| error(ErrorKind::DerivativeEvaluation, "missing sparsity shape"))?;
        let (mut jr, mut jc, mut j) = (
            vec![0; info.nnz_jac_g as usize],
            vec![0; info.nnz_jac_g as usize],
            vec![0.; info.nnz_jac_g as usize],
        );
        let (mut hr, mut hc, mut h) = (
            vec![0; info.nnz_h_lag as usize],
            vec![0; info.nnz_h_lag as usize],
            vec![0.; info.nnz_h_lag as usize],
        );
        let mut objective_gradient = vec![0.; p.n];
        if !t.eval_grad_f(&d.primal, true, &mut objective_gradient)
            || !t.eval_jac_g(
                None,
                false,
                SparsityRequest::Structure {
                    irow: &mut jr,
                    jcol: &mut jc,
                },
            )
            || !t.eval_jac_g(
                Some(&d.primal),
                true,
                SparsityRequest::Values { values: &mut j },
            )
            || !t.eval_h(
                None,
                false,
                1.,
                None,
                false,
                SparsityRequest::Structure {
                    irow: &mut hr,
                    jcol: &mut hc,
                },
            )
            || !t.eval_h(
                Some(&d.primal),
                true,
                1.,
                Some(&lambda),
                true,
                SparsityRequest::Values { values: &mut h },
            )
            || objective_gradient
                .iter()
                .chain(&j)
                .chain(&h)
                .any(|v| !v.is_finite())
        {
            return Err(error(
                ErrorKind::DerivativeEvaluation,
                "exact compiled derivative evaluation failed",
            ));
        }
        let mut entries = Vec::new();
        let mut jacobian = vec![Vec::new(); p.m];
        let mut stationarity = objective_gradient.clone();
        let mut stationarity_scale: Vec<_> =
            objective_gradient.iter().map(|v| 1. + v.abs()).collect();
        for ((r, c), v) in jr.into_iter().zip(jc).zip(j) {
            let (r, c) = (r as usize, c as usize);
            jacobian[r].push((c, v));
            if let Some(&a) = active.get(&r) {
                stationarity[c] += v * lambda[r];
                stationarity_scale[c] += (v * lambda[r]).abs();
                entries.push((p.n + a, c, v));
                entries.push((c, p.n + a, v));
            }
        }
        for &i in &bound_columns {
            // Finite variable bounds are not currently emitted by IVR, but
            // their stationarity terms must accompany any future bound rows.
            stationarity[i] += d.upper_bound_multipliers[i] - d.lower_bound_multipliers[i];
            stationarity_scale[i] +=
                d.upper_bound_multipliers[i].abs() + d.lower_bound_multipliers[i].abs();
        }
        if stationarity
            .iter()
            .zip(&stationarity_scale)
            .any(|(r, s)| !r.is_finite() || !s.is_finite() || r.abs() > duals::KKT_TOL * s)
        {
            return Err(McOpfSensitivityError {
                kind: ErrorKind::UnreliableDuals,
                message: "stationarity failed after removing inactive multipliers".into(),
                dual_status: Some(McOpfMarginalStatus::FailedKktCheck),
            });
        }
        for ((r, c), v) in hr.into_iter().zip(hc).zip(h) {
            let (r, c) = (r as usize, c as usize);
            entries.push((r, c, v));
            if r != c {
                entries.push((c, r, v));
            }
        }
        for (i, &c) in bound_columns.iter().enumerate() {
            let r = p.n + active.len() + i;
            entries.push((r, c, 1.));
            entries.push((c, r, 1.));
        }
        let factor = linear::Factor::new(dim, entries)?;
        Ok(Self {
            result,
            model,
            factor,
            active,
            jacobian,
            objective_gradient,
        })
    }

    /// All supported physical parameter identities for this result. Individual
    /// requests are still checked; unsupported entries are never silently zero.
    pub fn parameters(&self) -> Vec<McOpfParameter> {
        self.result
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .filter(|m| m.status == McOpfMarginalStatus::Conditional)
            .map(|m| m.parameter.clone())
            .collect()
    }

    /// One sparse backsolve for an arbitrary combined physical direction.
    pub fn forward(
        &self,
        direction: &[McOpfParameterDirection],
        outputs: &[McOpfSensitivityOutput],
    ) -> Result<Vec<McOpfOutputDerivative>> {
        let gradients: Vec<_> = outputs
            .iter()
            .map(|o| self.output_gradient(o))
            .collect::<Result<_>>()?;
        let mut rhs = vec![0.; self.factor.dimension()];
        for term in direction {
            if !term.delta.is_finite() {
                return Err(error(
                    ErrorKind::InvalidRequest,
                    "nonfinite parameter direction",
                ));
            }
            for (i, v) in self.parameter_rhs(&term.parameter)? {
                rhs[i] += term.delta * v;
            }
        }
        let tangent = self.factor.solve(&rhs)?;
        outputs
            .iter()
            .zip(gradients)
            .map(|(output, (gradient, unit))| {
                let derivative = dot(&gradient, &tangent)?;
                Ok(McOpfOutputDerivative {
                    output: output.clone(),
                    unit,
                    derivative,
                })
            })
            .collect()
    }

    /// One transpose backsolve for a weighted scalar output, followed by sparse
    /// contractions for each parameter. The KKT matrix is assembled symmetrically,
    /// so forward and transpose solves reuse the same factorization.
    pub fn adjoint(
        &self,
        weights: &[McOpfOutputWeight],
        parameters: &[McOpfParameter],
    ) -> Result<Vec<McOpfParameterDerivative>> {
        let columns: Vec<_> = parameters
            .iter()
            .map(|p| self.parameter_rhs(p))
            .collect::<Result<_>>()?;
        let mut rhs = vec![0.; self.factor.dimension()];
        for term in weights {
            if !term.weight.is_finite() {
                return Err(error(ErrorKind::InvalidRequest, "nonfinite output weight"));
            }
            let (gradient, _) = self.output_gradient(&term.output)?;
            for (r, g) in rhs.iter_mut().zip(gradient) {
                *r += term.weight * g;
            }
        }
        let adjoint = self.factor.solve(&rhs)?;
        parameters
            .iter()
            .zip(columns)
            .map(|(parameter, column)| {
                let derivative: f64 = column.iter().map(|&(i, v)| adjoint[i] * v).sum();
                finite(derivative)?;
                Ok(McOpfParameterDerivative {
                    parameter: parameter.clone(),
                    parameter_unit: parameter.unit(),
                    derivative,
                })
            })
            .collect()
    }

    fn parameter_rhs(&self, parameter: &McOpfParameter) -> Result<Vec<(usize, f64)>> {
        let d = self.result.duals.as_ref().unwrap();
        if !d
            .marginals
            .iter()
            .any(|m| &m.parameter == parameter && m.status == McOpfMarginalStatus::Conditional)
        {
            return Err(error(
                ErrorKind::UnsupportedParameter,
                format!("no regular parameter mapping for {parameter:?}"),
            ));
        }
        let terms = &self.model.dual_map.parameters[parameter];
        let mut rhs = BTreeMap::<usize, f64>::new();
        for (origin, rule) in terms {
            if matches!(rule, duals::Rule::Zero) {
                continue;
            }
            let r = self
                .model
                .dual_map
                .rows
                .iter()
                .position(|r| Some(*r) == *origin)
                .ok_or_else(|| {
                    error(ErrorKind::UnsupportedParameter, "parameter row was omitted")
                })?;
            let Some(&a) = self.active.get(&r) else {
                continue;
            };
            let row = &d.constraints[r];
            let bottom = self.model.problem.n + a;
            match rule {
                duals::Rule::Zero => {}
                duals::Rule::Equality { scale } => *rhs.entry(bottom).or_default() += scale,
                duals::Rule::Bound { scale, lower } => {
                    let bound = if *lower { row.lower } else { row.upper };
                    if bound.is_some_and(|b| {
                        (row.value - b).abs() <= duals::ACTIVE_TOL * (1. + b.abs())
                    }) {
                        *rhs.entry(bottom).or_default() += scale;
                    }
                }
                duals::Rule::Norm { physical } => {
                    // g(x,theta)=F(x)/theta^2. Both the constraint and the
                    // stationarity equation depend on theta. Omitting the
                    // mixed Jacobian term corrupts directions and adjoints.
                    *rhs.entry(bottom).or_default() += 2. * row.value / physical;
                    for &(c, j) in &self.jacobian[r] {
                        *rhs.entry(c).or_default() += 2. * row.multiplier * j / physical;
                    }
                }
                duals::Rule::Load(load) => {
                    let (value, gradient) = load
                        .evaluate(&d.primal)
                        .map_err(|e| error(ErrorKind::UnsupportedParameter, e))?;
                    *rhs.entry(bottom).or_default() -= value;
                    for (c, g) in gradient {
                        *rhs.entry(c).or_default() -= row.multiplier * g;
                    }
                }
                duals::Rule::Inverter(control) => {
                    let (value, gradient) = control
                        .evaluate(&d.primal)
                        .map_err(|e| error(ErrorKind::UnsupportedParameter, e))?;
                    *rhs.entry(bottom).or_default() -= value;
                    for (c, g) in gradient {
                        *rhs.entry(c).or_default() -= row.multiplier * g;
                    }
                }
            }
        }
        Ok(rhs.into_iter().collect())
    }
}
fn finite(value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(error(
            ErrorKind::LinearSolveFailure,
            "nonfinite sensitivity",
        ))
    }
}
fn dot(a: &[f64], b: &[f64]) -> Result<f64> {
    let value = a.iter().zip(b).map(|(a, b)| a * b).sum();
    finite(value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_rating_multiplier_tangent_includes_the_mixed_jacobian_term() {
        let mut net = crate::mc_opf::tests::rich_network();
        net.generators_mut()[0].cost = Some(vec![1.]);
        net.generators_mut()[0].p_max = Some(vec![150.]);
        net.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
        let solve = |n| {
            solve_mc_ac_opf_instance(
                Arc::new(McAcOpfInstance::from_network(n).unwrap()),
                &McOpfOptions {
                    collect_duals: true,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let r = solve(net.clone());
        let s = r.sensitivities().unwrap();
        let p = McOpfParameter::new(
            McOpfParameterOwner::Branch,
            "line:l",
            Some(0),
            McOpfParameterKind::CurrentRating,
        );
        let mut rhs = vec![0.; s.factor.dimension()];
        for (i, v) in s.parameter_rhs(&p).unwrap() {
            rhs[i] += v;
        }
        let tangent = s.factor.solve(&rhs).unwrap();
        let row = r
            .duals
            .as_ref()
            .unwrap()
            .constraints
            .iter()
            .position(|r| r.identity.label == "line:l:from:0:current")
            .unwrap();
        let a = s.active[&row];
        let mut plus = net.clone();
        let mut minus = net;
        plus.lines_mut()[0].i_max.as_mut().unwrap()[0] += 1e-5;
        minus.lines_mut()[0].i_max.as_mut().unwrap()[0] -= 1e-5;
        let lp = solve(plus).duals.unwrap().constraints[row].multiplier;
        let lm = solve(minus).duals.unwrap().constraints[row].multiplier;
        let fd = (lp - lm) / 2e-5;
        assert!((tangent[s.model.problem.n + a] - fd).abs() < 1e-6 + fd.abs() * 1e-3);
    }
}
