//! Serializable outer objectives and feasible interventions for a Study.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{ElementKey, Operand, Parameter, Power, Problem, SolveResponse, VoltageKind};

/// A weighted observable in the units reported by the selected formulation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ObservableWeight {
    pub element: ElementKey,
    pub weight: f64,
}

/// A scalar goal over the exact operating point and explicit interventions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StudyObjective {
    WeightedObservable {
        operand: Operand,
        #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 4096)))]
        weights: Vec<ObservableWeight>,
    },
    Sum {
        #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 4096)))]
        terms: Vec<StudyObjective>,
    },
    Scale {
        factor: f64,
        expression: Box<StudyObjective>,
    },
    SquaredTarget {
        target: f64,
        expression: Box<StudyObjective>,
    },
    InterventionPenalty {
        decision: String,
        linear: f64,
        quadratic: f64,
    },
}

/// A numerical intervention supported by the existing solvers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    BranchRating,
    ActiveDemand,
}

impl Intervention {
    pub fn parameter(self) -> Parameter {
        match self {
            Self::BranchRating => Parameter::LineLimit,
            Self::ActiveDemand => Parameter::Demand(Power::Active),
        }
    }

    pub fn units(self, formulation: Problem) -> &'static str {
        match (self, formulation) {
            (Self::BranchRating, Problem::Socwr) => "MVA",
            _ => "MW",
        }
    }
}

/// Bounds and increments apply to changes from the goal's anchor state in a Study.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionVariable {
    #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 128)))]
    pub id: String,
    pub element: ElementKey,
    pub intervention: Intervention,
    pub lower: f64,
    pub upper: f64,
    pub increment: f64,
    /// Budget consumption per absolute unit of intervention.
    pub budget_weight: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DemandConstraint {
    /// Allocate this additional MW across the declared demand decisions.
    Placement { increase_mw: f64 },
    /// Preserve the existing total MW across the declared demand decisions.
    Redistribution,
}

/// The feasible set, distinct from both the inner problem and the outer goal.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionSpace {
    #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 4096)))]
    pub variables: Vec<DecisionVariable>,
    #[cfg_attr(feature = "schema", schemars(range(min = 0)))]
    pub total_budget: f64,
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = 4096)))]
    pub max_changed_elements: usize,
    pub demand: Option<DemandConstraint>,
}

/// Scalar value and the complete chain-rule seed in served units.
#[derive(Clone, Debug)]
pub(crate) struct ObjectiveEvaluation {
    pub value: f64,
    pub observables: Vec<(Operand, ElementKey, f64)>,
    pub direct: BTreeMap<String, f64>,
}

impl ObjectiveEvaluation {
    fn constant(value: f64) -> Self {
        Self {
            value,
            observables: Vec::new(),
            direct: BTreeMap::new(),
        }
    }

    fn scale_derivative(&mut self, scale: f64) {
        for (_, _, weight) in &mut self.observables {
            *weight *= scale;
        }
        for value in self.direct.values_mut() {
            *value *= scale;
        }
    }

    fn finite(&self) -> bool {
        self.value.is_finite()
            && self.observables.iter().all(|(_, _, w)| w.is_finite())
            && self.direct.values().all(|v| v.is_finite())
    }
}

impl StudyObjective {
    pub fn validate(&self, space: &DecisionSpace) -> Result<(), String> {
        fn walk(
            expr: &StudyObjective,
            ids: &BTreeSet<&str>,
            depth: usize,
            count: &mut usize,
        ) -> Result<(), String> {
            *count += 1;
            if depth > 16 || *count > 4096 {
                return Err("objective exceeds 16 levels or 4096 terms".into());
            }
            match expr {
                StudyObjective::WeightedObservable { weights, .. } => {
                    *count += weights.len();
                    if weights.is_empty()
                        || *count > 4096
                        || weights.iter().any(|w| !w.weight.is_finite())
                    {
                        return Err(
                            "observable weights must be nonempty, finite and bounded".into()
                        );
                    }
                }
                StudyObjective::Sum { terms } => {
                    if terms.is_empty() {
                        return Err("objective sum requires a term".into());
                    }
                    for term in terms {
                        walk(term, ids, depth + 1, count)?;
                    }
                }
                StudyObjective::Scale { factor, expression } => {
                    if !factor.is_finite() {
                        return Err("objective scale must be finite".into());
                    }
                    walk(expression, ids, depth + 1, count)?;
                }
                StudyObjective::SquaredTarget { target, expression } => {
                    if !target.is_finite() {
                        return Err("objective target must be finite".into());
                    }
                    walk(expression, ids, depth + 1, count)?;
                }
                StudyObjective::InterventionPenalty {
                    decision,
                    linear,
                    quadratic,
                } => {
                    if !ids.contains(decision.as_str())
                        || !linear.is_finite()
                        || !quadratic.is_finite()
                    {
                        return Err("intervention penalty requires a declared decision and finite coefficients".into());
                    }
                }
            }
            Ok(())
        }
        let ids = space.variables.iter().map(|v| v.id.as_str()).collect();
        walk(self, &ids, 0, &mut 0)
    }

    pub(crate) fn evaluate(
        &self,
        response: &SolveResponse,
        network: &powerio::BalancedNetwork,
        interventions: &BTreeMap<String, f64>,
    ) -> Result<ObjectiveEvaluation, String> {
        let mut out = match self {
            Self::WeightedObservable { operand, weights } => {
                let mut out = ObjectiveEvaluation::constant(0.0);
                for term in weights {
                    let value = observable(response, network, *operand, &term.element)?;
                    out.value += term.weight * value;
                    let (seed, weight) = if *operand == Operand::Voltage(VoltageKind::Magnitude)
                        && response.formulation == Problem::Socwr
                    {
                        if value <= 0.0 {
                            return Err(
                                "voltage magnitude derivative is singular at zero squared voltage"
                                    .into(),
                            );
                        }
                        (
                            Operand::Voltage(VoltageKind::Squared),
                            term.weight / (2.0 * value),
                        )
                    } else {
                        (*operand, term.weight)
                    };
                    out.observables.push((seed, term.element.clone(), weight));
                }
                out
            }
            Self::Sum { terms } => {
                let mut out = ObjectiveEvaluation::constant(0.0);
                for term in terms {
                    let child = term.evaluate(response, network, interventions)?;
                    out.value += child.value;
                    out.observables.extend(child.observables);
                    for (key, value) in child.direct {
                        *out.direct.entry(key).or_default() += value;
                    }
                }
                out
            }
            Self::Scale { factor, expression } => {
                let mut child = expression.evaluate(response, network, interventions)?;
                child.value *= factor;
                child.scale_derivative(*factor);
                child
            }
            Self::SquaredTarget { target, expression } => {
                let mut child = expression.evaluate(response, network, interventions)?;
                let residual = child.value - target;
                child.value = residual * residual;
                child.scale_derivative(2.0 * residual);
                child
            }
            Self::InterventionPenalty {
                decision,
                linear,
                quadratic,
            } => {
                let u = interventions
                    .get(decision)
                    .ok_or_else(|| format!("missing intervention {decision}"))?;
                let mut out = ObjectiveEvaluation::constant(linear * u + quadratic * u * u);
                out.direct
                    .insert(decision.clone(), linear + 2.0 * quadratic * u);
                out
            }
        };
        if !out.finite() {
            return Err("objective or derivative overflowed; adjust weights and units".into());
        }
        out.observables.retain(|(_, _, w)| *w != 0.0);
        Ok(out)
    }
}

impl DecisionSpace {
    pub fn validate(&self, formulation: Problem) -> Result<(), String> {
        if self.variables.is_empty() || self.variables.len() > 4096 {
            return Err("decision space requires 1 to 4096 variables".into());
        }
        if !self.total_budget.is_finite()
            || self.total_budget < 0.0
            || self.max_changed_elements == 0
            || self.max_changed_elements > self.variables.len()
        {
            return Err("decision budget and cardinality are invalid".into());
        }
        let mut ids = BTreeSet::new();
        let mut targets = BTreeSet::new();
        let mut demands = 0;
        for v in &self.variables {
            if v.id.is_empty()
                || v.id.len() > 128
                || !ids.insert(&v.id)
                || !targets.insert((format!("{:?}", v.intervention), &v.element))
            {
                return Err("decision identities and targets must be nonempty and unique".into());
            }
            if [v.lower, v.upper, v.increment, v.budget_weight]
                .iter()
                .any(|x| !x.is_finite())
                || v.lower > v.upper
                || v.increment <= 0.0
                || v.budget_weight < 0.0
            {
                return Err(format!(
                    "invalid bounds, increment or budget weight for {}",
                    v.id
                ));
            }
            match v.intervention {
                Intervention::BranchRating
                    if !matches!(formulation, Problem::DcOpf | Problem::Socwr) =>
                {
                    return Err("rating planning requires a DC OPF or SOCWR formulation".into());
                }
                Intervention::ActiveDemand => demands += 1,
                _ => {}
            }
        }
        if matches!(formulation, Problem::DcPf | Problem::Acopf)
            || (formulation == Problem::Socwr && !cfg!(feature = "conic"))
        {
            return Err(
                "this build does not support differentiable planning for the formulation".into(),
            );
        }
        match &self.demand {
            Some(DemandConstraint::Placement { increase_mw }) => {
                if demands == 0
                    || !increase_mw.is_finite()
                    || *increase_mw <= 0.0
                    || self
                        .variables
                        .iter()
                        .any(|v| v.intervention == Intervention::ActiveDemand && v.lower < 0.0)
                {
                    return Err("placement requires demand variables, a positive total and nonnegative changes".into());
                }
            }
            Some(DemandConstraint::Redistribution) if demands < 2 => {
                return Err("redistribution requires at least two demand decisions".into());
            }
            None if demands > 0 => {
                return Err("demand planning requires an explicit total constraint".into())
            }
            _ => {}
        }
        Ok(())
    }

    pub fn feasible(&self, changes: &[f64], tolerance: f64) -> bool {
        if changes.len() != self.variables.len() || !tolerance.is_finite() || tolerance < 0.0 {
            return false;
        }
        let mut budget = 0.0;
        let mut count = 0;
        let mut demand = 0.0;
        for (v, &x) in self.variables.iter().zip(changes) {
            if !x.is_finite()
                || x < v.lower - tolerance
                || x > v.upper + tolerance
                || (x / v.increment - (x / v.increment).round()).abs() * v.increment > tolerance
            {
                return false;
            }
            budget += v.budget_weight * x.abs();
            count += usize::from(x.abs() > tolerance);
            if v.intervention == Intervention::ActiveDemand {
                demand += x;
            }
        }
        let total = match self.demand {
            Some(DemandConstraint::Placement { increase_mw }) => increase_mw,
            _ => 0.0,
        };
        budget <= self.total_budget + tolerance
            && count <= self.max_changed_elements
            && (demand - total).abs() <= tolerance
    }
}

pub(crate) fn source_element_id(
    network: &powerio::BalancedNetwork,
    axis: crate::Axis,
    key: &ElementKey,
) -> Result<usize, String> {
    match key {
        ElementKey::Id(id) => {
            usize::try_from(*id).map_err(|_| "element id must be nonnegative".into())
        }
        ElementKey::Uid(uid) => {
            let found = match axis {
                crate::Axis::Bus => network
                    .buses()
                    .iter()
                    .find(|b| b.uid.as_deref() == Some(uid))
                    .map(|b| b.id.0),
                crate::Axis::Branch => network
                    .branches()
                    .iter()
                    .position(|b| b.uid.as_deref() == Some(uid))
                    .map(|i| i + 1),
                crate::Axis::Generator => network
                    .generators()
                    .iter()
                    .position(|g| g.uid.as_deref() == Some(uid))
                    .map(|i| i + 1),
            };
            found.ok_or_else(|| format!("unknown {axis:?} identity {uid}"))
        }
    }
}

fn observable(
    response: &SolveResponse,
    network: &powerio::BalancedNetwork,
    operand: Operand,
    key: &ElementKey,
) -> Result<f64, String> {
    let id = source_element_id(network, operand.axis(), key)?;
    let bus = |values: &Option<Vec<crate::BusScalar>>| {
        values
            .as_ref()
            .and_then(|v| v.iter().find(|v| v.bus == id))
            .map(|v| v.value)
    };
    let value = match operand {
        Operand::Price(Power::Active) => bus(&response.lmp),
        Operand::Price(Power::Reactive) => bus(&response.lmp_q),
        Operand::Voltage(VoltageKind::Magnitude) => bus(&response.vm),
        Operand::Voltage(VoltageKind::Angle) => bus(&response.va),
        Operand::Voltage(VoltageKind::Squared) => bus(&response.w),
        Operand::Voltage(kind @ (VoltageKind::ProductReal | VoltageKind::ProductImag)) => {
            let products = if kind == VoltageKind::ProductReal {
                &response.wr
            } else {
                &response.wi
            };
            products
                .as_ref()
                .and_then(|v| v.iter().find(|v| v.branch == id))
                .map(|v| v.value)
        }
        Operand::Dispatch(power) => response
            .dispatch
            .as_ref()
            .and_then(|v| v.iter().find(|g| g.gen == id))
            .and_then(|g| match power {
                Power::Active => Some(g.pg),
                Power::Reactive => g.qg,
            }),
        Operand::Flow { power, end } => response
            .flows
            .as_ref()
            .and_then(|v| v.iter().find(|b| b.branch == id))
            .and_then(|b| match (power, end) {
                (Power::Active, crate::End::From) => Some(b.pf),
                (Power::Active, crate::End::To) => {
                    b.pt.or_else(|| (response.formulation == Problem::DcOpf).then_some(-b.pf))
                }
                (Power::Reactive, crate::End::From) => b.qf,
                (Power::Reactive, crate::End::To) => b.qt,
            }),
    };
    value
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{operand:?} is unavailable for {key} in this exact solution"))
}

/// A derivative in objective units per MW or MVA of the named decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionDerivative {
    pub decision: String,
    pub value: f64,
    pub units: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DerivativeMethod {
    CombinedAdjoint,
}

/// Settings used for the derivative solve; these do not alter the inner objective.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DerivativeNumerics {
    pub method: DerivativeMethod,
    pub regularization: f64,
    pub refinement_iterations: usize,
    pub residual_tolerance_factor: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ObjectiveResult {
    pub value: f64,
    pub gradient: Vec<DecisionDerivative>,
    pub local_only: bool,
    pub numerics: DerivativeNumerics,
}
