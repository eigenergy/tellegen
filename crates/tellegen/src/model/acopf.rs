//! Exact polar AC OPF expressions compiled from PowerIO's canonical preparation.
//!
//! This module deliberately stops at the private nonlinear-program boundary.
//! Solver status mapping and portable solution emission belong to the next
//! layer, while all electrical semantics and stable source/index maps live here.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use pounce_nl::nl_reader::{BinOp, Expr, NlProblem, NlProblemParts, NlTnlp, UnaryOp};
use pounce_rs::{ApplicationReturnStatus, IpoptApplication, TNLP};
use powerio::LoadVoltageModel;
use powerio_matrix::{
    build_ac_opf_preparation, AcOpfAssemblyOptions, AcOpfPreparation, PreparedObjective, Units,
};
use powerio_prob::AcOpfInstance;
use sha2::{Digest, Sha256};

use super::{reject_unsupported_active_elements, validate_canonical_identity, PiecewiseCost};

const INF: f64 = 1.0e19;
const FORMULATION_TAG: &[u8] = b"tellegen/acopf/polar-v1";
const PRIMAL_TOLERANCE: f64 = 1.0e-6;
const POUNCE_OPTIONS: &str = "linear_solver feral
hessian_approximation exact
nlp_scaling_method none
linear_system_scaling none
tol 1e-8
constr_viol_tol 1e-8
acceptable_tol 1e-7
max_iter 1000
print_level 0
";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AcOpfResidualCheck {
    pub(crate) active_balance: f64,
    pub(crate) reactive_balance: f64,
    pub(crate) variable_bounds: f64,
    pub(crate) reference_angles: f64,
    pub(crate) angle_limits: f64,
    pub(crate) thermal_limits: f64,
    pub(crate) piecewise_epigraph: f64,
    pub(crate) objective: f64,
}

impl AcOpfResidualCheck {
    fn max_violation(self) -> f64 {
        self.active_balance
            .max(self.reactive_balance)
            .max(self.variable_bounds)
            .max(self.reference_angles)
            .max(self.angle_limits)
            .max(self.thermal_limits)
            .max(self.piecewise_epigraph)
            .max(self.objective)
    }
}

/// A converged POUNCE point after an independent calculation from PowerIO's
/// prepared arrays. Duals intentionally stay private and unpublished until a
/// perturbation test establishes their sign and source-unit scaling.
#[derive(Clone, Debug)]
pub(crate) struct AcOpfSolved {
    pub(crate) preparation: AcOpfPreparation,
    pub(crate) va: Vec<f64>,
    pub(crate) vm: Vec<f64>,
    pub(crate) pg: Vec<f64>,
    pub(crate) qg: Vec<f64>,
    pub(crate) p_injection: Vec<f64>,
    pub(crate) q_injection: Vec<f64>,
    pub(crate) p_from: Vec<f64>,
    pub(crate) q_from: Vec<f64>,
    pub(crate) p_to: Vec<f64>,
    pub(crate) q_to: Vec<f64>,
    pub(crate) objective: f64,
    pub(crate) residuals: AcOpfResidualCheck,
    pub(crate) iterations: i32,
    pub(crate) solver_status: ApplicationReturnStatus,
    pub(crate) fingerprint: String,
}

/// The recorded assembly policy for the canonical model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AcOpfAssemblyPolicy {
    pub(super) units: Units,
    pub(super) skip_zero_impedance: bool,
    pub(super) synthesize_unrated_limits: bool,
    pub(super) correct_angle_difference_bounds: bool,
}

impl AcOpfAssemblyPolicy {
    const CANONICAL: Self = Self {
        units: Units::PerUnit,
        skip_zero_impedance: false,
        synthesize_unrated_limits: false,
        correct_angle_difference_bounds: true,
    };

    fn options(self) -> AcOpfAssemblyOptions {
        AcOpfAssemblyOptions::default()
            .with_units(self.units)
            .with_skip_zero_impedance(self.skip_zero_impedance)
            .with_synthesize_unrated_limits(self.synthesize_unrated_limits)
            .with_correct_angle_difference_bounds(self.correct_angle_difference_bounds)
    }
}

/// Stable variable columns. Every vector is indexed in PowerIO preparation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AcOpfColumns {
    pub(super) va: Vec<usize>,
    pub(super) vm: Vec<usize>,
    pub(super) pg: Vec<usize>,
    pub(super) qg: Vec<usize>,
    /// One entry per generator; `None` for a quadratic/no-cost generator.
    pub(super) piecewise_epigraph: Vec<Option<usize>>,
}

/// Stable constraint rows. Optional branch rows preserve branch-column alignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AcOpfRows {
    pub(super) p_balance: Vec<usize>,
    pub(super) q_balance: Vec<usize>,
    pub(super) reference_angle: Vec<usize>,
    pub(super) angle_difference: Vec<Option<usize>>,
    pub(super) thermal_from: Vec<Option<usize>>,
    pub(super) thermal_to: Vec<Option<usize>>,
    /// Segment rows for each generator's exact convex piecewise epigraph.
    pub(super) piecewise_segments: Vec<Vec<usize>>,
}

/// The private exact model plus everything later solve/emission code needs to
/// map POUNCE vectors back to PowerIO identities.
#[derive(Clone, Debug)]
pub(super) struct CanonicalAcOpfModel {
    pub(super) preparation: AcOpfPreparation,
    pub(super) policy: AcOpfAssemblyPolicy,
    pub(super) columns: AcOpfColumns,
    pub(super) rows: AcOpfRows,
    pub(super) fingerprint: String,
    problem: NlProblem,
}

impl CanonicalAcOpfModel {
    pub(super) fn tnlp(&self) -> Result<NlTnlp, String> {
        NlTnlp::try_new(self.problem.clone())
    }

    pub(super) fn problem(&self) -> &NlProblem {
        &self.problem
    }
}

#[derive(Default)]
struct RowsBuilder {
    expressions: Vec<Expr>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    names: Vec<String>,
}

impl RowsBuilder {
    fn push(&mut self, name: String, expression: Expr, lower: f64, upper: f64) -> usize {
        let row = self.expressions.len();
        self.expressions.push(expression);
        self.lower.push(lower);
        self.upper.push(upper);
        self.names.push(name);
        row
    }
}

#[derive(Clone)]
struct BranchFlow {
    p_from: Expr,
    q_from: Expr,
    p_to: Expr,
    q_to: Expr,
}

fn constant(value: f64) -> Expr {
    Expr::Const(value)
}

fn variable(index: usize) -> Expr {
    Expr::Var(index)
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary(op, Box::new(lhs), Box::new(rhs))
}

fn add(lhs: Expr, rhs: Expr) -> Expr {
    binary(BinOp::Add, lhs, rhs)
}

fn subtract(lhs: Expr, rhs: Expr) -> Expr {
    binary(BinOp::Sub, lhs, rhs)
}

fn multiply(lhs: Expr, rhs: Expr) -> Expr {
    binary(BinOp::Mul, lhs, rhs)
}

fn scaled(scale: f64, value: Expr) -> Expr {
    multiply(constant(scale), value)
}

fn square(value: Expr) -> Expr {
    binary(BinOp::Pow, value, constant(2.0))
}

fn unary(op: UnaryOp, value: Expr) -> Expr {
    Expr::Unary(op, Box::new(value))
}

fn sum(terms: Vec<Expr>) -> Expr {
    match terms.len() {
        0 => constant(0.0),
        1 => terms.into_iter().next().expect("one term"),
        _ => Expr::Sum(terms),
    }
}

fn cse(value: Expr) -> Expr {
    Expr::Cse(Arc::new(value))
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn clamp_start(value: f64, lower: f64, upper: f64) -> f64 {
    finite_or(value, 0.0).clamp(lower, upper)
}

fn element_name(kind: &str, row: usize, uid: Option<&str>) -> String {
    uid.map_or_else(
        || format!("{kind}[{row}]"),
        |uid| format!("{kind} \"{uid}\""),
    )
}

fn validate_supported(instance: &AcOpfInstance) -> Result<(), String> {
    let network = instance.network();
    validate_canonical_identity(network)?;

    if let Some((row, storage)) = network
        .storage()
        .iter()
        .enumerate()
        .find(|(_, storage)| storage.in_service)
    {
        return Err(format!(
            "{} is in service; canonical AC OPF does not model storage behavior",
            element_name("storage", row, storage.uid.as_deref())
        ));
    }
    if let Some((row, load)) = network.loads().iter().enumerate().find(|(_, load)| {
        load.in_service
            && !matches!(
                load.voltage_model.as_ref(),
                None | Some(LoadVoltageModel::ConstantPower)
            )
    }) {
        return Err(format!(
            "{} declares voltage-dependent demand; canonical AC OPF supports constant-power loads only",
            element_name("load", row, load.uid.as_deref())
        ));
    }
    if let Some((row, generator)) =
        network
            .generators()
            .iter()
            .enumerate()
            .find(|(_, generator)| {
                generator.in_service
                    && generator.voltage_regulation_on
                    && generator
                        .regulated_bus
                        .is_some_and(|regulated| regulated != generator.bus)
            })
    {
        return Err(format!(
            "{} regulates a remote bus; canonical AC OPF supports local voltage control only",
            element_name("generator", row, generator.uid.as_deref())
        ));
    }

    reject_unsupported_active_elements(network)
}

fn branch_flow(prep: &AcOpfPreparation, columns: &AcOpfColumns, branch: usize) -> BranchFlow {
    let f = prep.branches.from_bus[branch];
    let t = prep.branches.to_bus[branch];
    let g = prep.branches.g[branch];
    let b = prep.branches.b[branch];
    let tap = prep.branches.tap[branch];
    let shift = prep.branches.shift[branch];
    let tap_squared = tap * tap;
    let tr = tap * shift.cos();
    let ti = tap * shift.sin();

    let vf = variable(columns.vm[f]);
    let vt = variable(columns.vm[t]);
    let vf_squared = cse(square(vf.clone()));
    let vt_squared = cse(square(vt.clone()));
    let angle = subtract(variable(columns.va[f]), variable(columns.va[t]));
    let voltage_product = multiply(vf, vt);
    let wr = cse(multiply(
        voltage_product.clone(),
        unary(UnaryOp::Cos, angle.clone()),
    ));
    let wi = cse(multiply(voltage_product, unary(UnaryOp::Sin, angle)));

    let p_from = cse(sum(vec![
        scaled(
            (g + prep.branches.g_fr[branch]) / tap_squared,
            vf_squared.clone(),
        ),
        scaled((-g * tr + b * ti) / tap_squared, wr.clone()),
        scaled((-b * tr - g * ti) / tap_squared, wi.clone()),
    ]));
    let q_from = cse(sum(vec![
        scaled(-(b + prep.branches.b_fr[branch]) / tap_squared, vf_squared),
        scaled((b * tr + g * ti) / tap_squared, wr.clone()),
        scaled((-g * tr + b * ti) / tap_squared, wi.clone()),
    ]));
    let p_to = cse(sum(vec![
        scaled(g + prep.branches.g_to[branch], vt_squared.clone()),
        scaled((-g * tr - b * ti) / tap_squared, wr.clone()),
        scaled((b * tr - g * ti) / tap_squared, wi.clone()),
    ]));
    let q_to = cse(sum(vec![
        scaled(-(b + prep.branches.b_to[branch]), vt_squared),
        scaled((b * tr - g * ti) / tap_squared, wr),
        scaled((g * tr + b * ti) / tap_squared, wi),
    ]));

    BranchFlow {
        p_from,
        q_from,
        p_to,
        q_to,
    }
}

/// Compile the exact canonical polar model. The formulation uses these row signs:
/// nodal injection minus withdrawal equals zero; thermal rows are nonnegative
/// remaining squared MVA margin; piecewise rows are nonnegative epigraph margin.
pub(super) fn compile_ac_opf_model(
    instance: &AcOpfInstance,
) -> Result<CanonicalAcOpfModel, String> {
    validate_supported(instance)?;
    let policy = AcOpfAssemblyPolicy::CANONICAL;
    let prep = build_ac_opf_preparation(instance, &policy.options())
        .map_err(|error| format!("{}: {error}", error.code().code))?;
    if let Some(identity) = prep.storage.identities.first() {
        return Err(format!(
            "storage \"{identity}\" reached AC OPF preparation; storage behavior is unsupported"
        ));
    }

    let n = prep.n_buses;
    let k = prep.n_generators();
    let m = prep.n_branches();
    let va = (0..n).collect::<Vec<_>>();
    let vm = (n..2 * n).collect::<Vec<_>>();
    let pg = (2 * n..2 * n + k).collect::<Vec<_>>();
    let qg = (2 * n + k..2 * n + 2 * k).collect::<Vec<_>>();
    let mut next_column = 2 * n + 2 * k;
    let piecewise_epigraph = prep
        .generators
        .piecewise_linear
        .iter()
        .map(|cost| {
            (prep.objective == PreparedObjective::NetworkGeneratorCost && cost.is_some()).then(
                || {
                    let column = next_column;
                    next_column += 1;
                    column
                },
            )
        })
        .collect::<Vec<_>>();
    let columns = AcOpfColumns {
        va,
        vm,
        pg,
        qg,
        piecewise_epigraph,
    };

    let mut x_l = vec![-INF; next_column];
    let mut x_u = vec![INF; next_column];
    let mut x0 = vec![0.0; next_column];
    let mut var_names = vec![String::new(); next_column];
    let vm_start = prep.calc_vm_setpoints();
    for bus in 0..n {
        var_names[columns.va[bus]] = format!("va[{}]", prep.bus_ids[bus].0);
        x0[columns.va[bus]] = finite_or(prep.buses.initial_va[bus], 0.0);

        let vm_column = columns.vm[bus];
        var_names[vm_column] = format!("vm[{}]", prep.bus_ids[bus].0);
        x_l[vm_column] = 0.0;
        if prep.buses.voltage_bound_active[bus] {
            x_l[vm_column] = prep.buses.vm_min[bus];
            x_u[vm_column] = prep.buses.vm_max[bus];
        }
        x0[vm_column] = clamp_start(vm_start[bus], x_l[vm_column], x_u[vm_column]);
    }
    for generator in 0..k {
        let identity = &prep.generators.identities[generator];
        let p_column = columns.pg[generator];
        let q_column = columns.qg[generator];
        var_names[p_column] = format!("pg[{identity}]");
        var_names[q_column] = format!("qg[{identity}]");
        if prep.generators.capability_active[generator] {
            x_l[p_column] = prep.generators.pmin[generator];
            x_u[p_column] = prep.generators.pmax[generator];
            x_l[q_column] = prep.generators.qmin[generator];
            x_u[q_column] = prep.generators.qmax[generator];
        }
        x0[p_column] = clamp_start(prep.generators.pg[generator], x_l[p_column], x_u[p_column]);
        x0[q_column] = clamp_start(prep.generators.qg[generator], x_l[q_column], x_u[q_column]);
    }

    let mut objective_terms = Vec::new();
    let mut objective_constant = 0.0;
    let mut piecewise_costs = vec![None; k];
    if prep.objective == PreparedObjective::NetworkGeneratorCost {
        for generator in 0..k {
            if let Some(cost) = prep.generators.piecewise_linear[generator].clone() {
                let cost = PiecewiseCost::from_prepared(cost);
                let epigraph = columns.piecewise_epigraph[generator].expect("piecewise column");
                var_names[epigraph] =
                    format!("cost_epigraph[{}]", prep.generators.identities[generator]);
                x0[epigraph] = cost.evaluate(x0[columns.pg[generator]]);
                objective_terms.push(variable(epigraph));
                piecewise_costs[generator] = Some(cost);
            } else {
                let p = variable(columns.pg[generator]);
                objective_terms.push(sum(vec![
                    scaled(0.5 * prep.generators.q[generator], square(p.clone())),
                    scaled(prep.generators.c[generator], p),
                ]));
                objective_constant += prep.generators.c0[generator];
            }
        }
    }

    let mut p_terms = (0..n)
        .map(|bus| {
            vec![
                constant(-prep.buses.p_d[bus]),
                scaled(-prep.buses.g_s[bus], square(variable(columns.vm[bus]))),
            ]
        })
        .collect::<Vec<_>>();
    let mut q_terms = (0..n)
        .map(|bus| {
            vec![
                constant(-prep.buses.q_d[bus]),
                scaled(prep.buses.b_s[bus], square(variable(columns.vm[bus]))),
            ]
        })
        .collect::<Vec<_>>();
    for generator in 0..k {
        let bus = prep.generators.bus_of_gen[generator];
        p_terms[bus].push(variable(columns.pg[generator]));
        q_terms[bus].push(variable(columns.qg[generator]));
    }
    let flows = (0..m)
        .map(|branch| branch_flow(&prep, &columns, branch))
        .collect::<Vec<_>>();
    for (branch, flow) in flows.iter().enumerate() {
        let f = prep.branches.from_bus[branch];
        let t = prep.branches.to_bus[branch];
        p_terms[f].push(scaled(-1.0, flow.p_from.clone()));
        q_terms[f].push(scaled(-1.0, flow.q_from.clone()));
        p_terms[t].push(scaled(-1.0, flow.p_to.clone()));
        q_terms[t].push(scaled(-1.0, flow.q_to.clone()));
    }

    let mut rows_builder = RowsBuilder::default();
    let p_balance = (0..n)
        .map(|bus| {
            rows_builder.push(
                format!("p_balance[{}]", prep.bus_ids[bus].0),
                sum(std::mem::take(&mut p_terms[bus])),
                0.0,
                0.0,
            )
        })
        .collect::<Vec<_>>();
    let q_balance = (0..n)
        .map(|bus| {
            rows_builder.push(
                format!("q_balance[{}]", prep.bus_ids[bus].0),
                sum(std::mem::take(&mut q_terms[bus])),
                0.0,
                0.0,
            )
        })
        .collect::<Vec<_>>();
    let reference_angle = prep
        .reference_buses
        .iter()
        .map(|&bus| {
            rows_builder.push(
                format!("reference_angle[{}]", prep.bus_ids[bus].0),
                variable(columns.va[bus]),
                0.0,
                0.0,
            )
        })
        .collect::<Vec<_>>();
    let angle_difference = (0..m)
        .map(|branch| {
            prep.branches.angle_bound_active[branch].then(|| {
                let f = prep.branches.from_bus[branch];
                let t = prep.branches.to_bus[branch];
                rows_builder.push(
                    format!("angle_difference[{}]", prep.branches.identities[branch]),
                    subtract(variable(columns.va[f]), variable(columns.va[t])),
                    prep.branches.angle_min[branch],
                    prep.branches.angle_max[branch],
                )
            })
        })
        .collect::<Vec<_>>();
    let mut thermal_from = vec![None; m];
    let mut thermal_to = vec![None; m];
    for branch in 0..m {
        if prep.branches.thermal_limit_active[branch] && prep.branches.s_max[branch] > 0.0 {
            let limit_squared = prep.branches.s_max[branch].powi(2);
            thermal_from[branch] = Some(rows_builder.push(
                format!("thermal_from[{}]", prep.branches.identities[branch]),
                subtract(
                    constant(limit_squared),
                    add(
                        square(flows[branch].p_from.clone()),
                        square(flows[branch].q_from.clone()),
                    ),
                ),
                0.0,
                INF,
            ));
            thermal_to[branch] = Some(rows_builder.push(
                format!("thermal_to[{}]", prep.branches.identities[branch]),
                subtract(
                    constant(limit_squared),
                    add(
                        square(flows[branch].p_to.clone()),
                        square(flows[branch].q_to.clone()),
                    ),
                ),
                0.0,
                INF,
            ));
        }
    }
    let mut piecewise_segments = vec![Vec::new(); k];
    for generator in 0..k {
        if let Some(cost) = &piecewise_costs[generator] {
            let epigraph = columns.piecewise_epigraph[generator].expect("piecewise column");
            for segment in 0..cost.segment_count() {
                let row = rows_builder.push(
                    format!(
                        "piecewise_cost[{}][{segment}]",
                        prep.generators.identities[generator]
                    ),
                    subtract(
                        variable(epigraph),
                        add(
                            scaled(cost.slopes[segment], variable(columns.pg[generator])),
                            constant(cost.intercepts[segment]),
                        ),
                    ),
                    0.0,
                    INF,
                );
                piecewise_segments[generator].push(row);
            }
        }
    }
    let rows = AcOpfRows {
        p_balance,
        q_balance,
        reference_angle,
        angle_difference,
        thermal_from,
        thermal_to,
        piecewise_segments,
    };

    let problem = NlProblem::from_expressions(NlProblemParts {
        minimize: true,
        objective: sum(objective_terms),
        obj_constant: objective_constant,
        constraints: rows_builder.expressions,
        x_l,
        x_u,
        x0,
        g_l: rows_builder.lower,
        g_u: rows_builder.upper,
        var_names,
        con_names: rows_builder.names,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(FORMULATION_TAG);
    hasher.update(
        serde_json::to_vec(&prep)
            .map_err(|error| format!("could not fingerprint AC OPF preparation: {error}"))?,
    );
    let fingerprint = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("");

    Ok(CanonicalAcOpfModel {
        preparation: prep,
        policy,
        columns,
        rows,
        fingerprint,
        problem,
    })
}

fn numeric_branch_flow(prep: &AcOpfPreparation, va: &[f64], vm: &[f64], branch: usize) -> [f64; 4] {
    let f = prep.branches.from_bus[branch];
    let t = prep.branches.to_bus[branch];
    let delta = va[f] - va[t];
    let wr = vm[f] * vm[t] * delta.cos();
    let wi = vm[f] * vm[t] * delta.sin();
    let g = prep.branches.g[branch];
    let b = prep.branches.b[branch];
    let tap = prep.branches.tap[branch];
    let tr = tap * prep.branches.shift[branch].cos();
    let ti = tap * prep.branches.shift[branch].sin();
    let tm2 = tap * tap;
    let p_from = (g + prep.branches.g_fr[branch]) / tm2 * vm[f].powi(2)
        + (-g * tr + b * ti) / tm2 * wr
        + (-b * tr - g * ti) / tm2 * wi;
    let q_from = -(b + prep.branches.b_fr[branch]) / tm2 * vm[f].powi(2)
        + (b * tr + g * ti) / tm2 * wr
        + (-g * tr + b * ti) / tm2 * wi;
    let p_to = (g + prep.branches.g_to[branch]) * vm[t].powi(2)
        + (-g * tr - b * ti) / tm2 * wr
        + (b * tr - g * ti) / tm2 * wi;
    let q_to = -(b + prep.branches.b_to[branch]) * vm[t].powi(2)
        + (b * tr - g * ti) / tm2 * wr
        + (g * tr + b * ti) / tm2 * wi;
    [p_from, q_from, p_to, q_to]
}

fn piecewise_value(cost: &powerio_matrix::PiecewiseLinearCost, power: f64) -> f64 {
    (0..cost.power.len() - 1)
        .map(|segment| {
            let slope = (cost.value[segment + 1] - cost.value[segment])
                / (cost.power[segment + 1] - cost.power[segment]);
            slope * power + cost.value[segment] - slope * cost.power[segment]
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn positive_violation(value: f64, lower: f64, upper: f64) -> f64 {
    (lower - value).max(0.0).max((value - upper).max(0.0))
}

fn validate_primal(
    model: &CanonicalAcOpfModel,
    x: &[f64],
    solver_objective: f64,
    iterations: i32,
    solver_status: ApplicationReturnStatus,
) -> Result<AcOpfSolved, String> {
    let prep = &model.preparation;
    if x.len() != model.problem.n || x.iter().any(|value| !value.is_finite()) {
        return Err("POUNCE returned a missing, non-finite, or wrong-length primal point".into());
    }
    let va = model
        .columns
        .va
        .iter()
        .map(|&column| x[column])
        .collect::<Vec<_>>();
    let vm = model
        .columns
        .vm
        .iter()
        .map(|&column| x[column])
        .collect::<Vec<_>>();
    let pg = model
        .columns
        .pg
        .iter()
        .map(|&column| x[column])
        .collect::<Vec<_>>();
    let qg = model
        .columns
        .qg
        .iter()
        .map(|&column| x[column])
        .collect::<Vec<_>>();

    let flows = (0..prep.n_branches())
        .map(|branch| numeric_branch_flow(prep, &va, &vm, branch))
        .collect::<Vec<_>>();
    let p_from = flows.iter().map(|flow| flow[0]).collect::<Vec<_>>();
    let q_from = flows.iter().map(|flow| flow[1]).collect::<Vec<_>>();
    let p_to = flows.iter().map(|flow| flow[2]).collect::<Vec<_>>();
    let q_to = flows.iter().map(|flow| flow[3]).collect::<Vec<_>>();

    let mut p_injection = (0..prep.n_buses)
        .map(|bus| -prep.buses.p_d[bus] - prep.buses.g_s[bus] * vm[bus].powi(2))
        .collect::<Vec<_>>();
    let mut q_injection = (0..prep.n_buses)
        .map(|bus| -prep.buses.q_d[bus] + prep.buses.b_s[bus] * vm[bus].powi(2))
        .collect::<Vec<_>>();
    for generator in 0..prep.n_generators() {
        let bus = prep.generators.bus_of_gen[generator];
        p_injection[bus] += pg[generator];
        q_injection[bus] += qg[generator];
    }
    let mut p_balance = p_injection.clone();
    let mut q_balance = q_injection.clone();
    for (branch, flow) in flows.iter().enumerate() {
        let f = prep.branches.from_bus[branch];
        let t = prep.branches.to_bus[branch];
        p_balance[f] -= flow[0];
        q_balance[f] -= flow[1];
        p_balance[t] -= flow[2];
        q_balance[t] -= flow[3];
    }

    let variable_bounds = x
        .iter()
        .zip(&model.problem.x_l)
        .zip(&model.problem.x_u)
        .map(|((&value, &lower), &upper)| positive_violation(value, lower, upper))
        .fold(0.0, f64::max);
    let reference_angles = prep
        .reference_buses
        .iter()
        .map(|&bus| va[bus].abs())
        .fold(0.0, f64::max);
    let angle_limits = (0..prep.n_branches())
        .filter(|&branch| prep.branches.angle_bound_active[branch])
        .map(|branch| {
            let angle = va[prep.branches.from_bus[branch]] - va[prep.branches.to_bus[branch]];
            positive_violation(
                angle,
                prep.branches.angle_min[branch],
                prep.branches.angle_max[branch],
            )
        })
        .fold(0.0, f64::max);
    let thermal_limits = flows
        .iter()
        .enumerate()
        .filter(|(branch, _)| {
            prep.branches.thermal_limit_active[*branch] && prep.branches.s_max[*branch] > 0.0
        })
        .map(|(branch, flow)| {
            (flow[0].hypot(flow[1]).max(flow[2].hypot(flow[3])) - prep.branches.s_max[branch])
                .max(0.0)
        })
        .fold(0.0, f64::max);

    let mut independent_objective = 0.0;
    let mut piecewise_epigraph: f64 = 0.0;
    if prep.objective == PreparedObjective::NetworkGeneratorCost {
        for generator in 0..prep.n_generators() {
            if let Some(cost) = &prep.generators.piecewise_linear[generator] {
                let value = piecewise_value(cost, pg[generator]);
                independent_objective += value;
                let epigraph = x[model.columns.piecewise_epigraph[generator]
                    .expect("piecewise objective has an epigraph column")];
                piecewise_epigraph = piecewise_epigraph.max((value - epigraph).max(0.0));
            } else {
                independent_objective += 0.5 * prep.generators.q[generator] * pg[generator].powi(2)
                    + prep.generators.c[generator] * pg[generator]
                    + prep.generators.c0[generator];
            }
        }
    }
    let all_derived_finite = va
        .iter()
        .chain(&vm)
        .chain(&pg)
        .chain(&qg)
        .chain(&p_injection)
        .chain(&q_injection)
        .chain(&p_from)
        .chain(&q_from)
        .chain(&p_to)
        .chain(&q_to)
        .all(|value| value.is_finite());
    if !all_derived_finite || !solver_objective.is_finite() || !independent_objective.is_finite() {
        return Err(
            "POUNCE AC OPF point produced a non-finite independently calculated output".into(),
        );
    }
    let objective_scale = independent_objective.abs().max(1.0);
    let residuals = AcOpfResidualCheck {
        active_balance: p_balance
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        reactive_balance: q_balance
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        variable_bounds,
        reference_angles,
        angle_limits,
        thermal_limits,
        piecewise_epigraph,
        objective: (solver_objective - independent_objective).abs() / objective_scale,
    };
    if residuals.max_violation() > PRIMAL_TOLERANCE {
        return Err(format!(
            "POUNCE returned {solver_status:?}, but independent AC validation failed (max violation {:.3e}; residuals {residuals:?})",
            residuals.max_violation()
        ));
    }

    Ok(AcOpfSolved {
        preparation: prep.clone(),
        va,
        vm,
        pg,
        qg,
        p_injection,
        q_injection,
        p_from,
        q_from,
        p_to,
        q_to,
        objective: independent_objective,
        residuals,
        iterations,
        solver_status,
        fingerprint: model.fingerprint.clone(),
    })
}

/// Compile and solve one canonical instance with exact POUNCE derivatives.
/// A local infeasibility report is returned as a diagnostic error, never
/// promoted to PowerIO's proof-strength `Termination::Infeasible`.
pub(crate) fn solve_ac_opf(instance: &AcOpfInstance) -> Result<AcOpfSolved, String> {
    let model = compile_ac_opf_model(instance)?;
    let tnlp = Rc::new(RefCell::new(model.tnlp()?));
    let mut application = IpoptApplication::new();
    application
        .initialize_with_options_str(POUNCE_OPTIONS)
        .map_err(|error| format!("could not configure POUNCE AC OPF: {error}"))?;
    application
        .initialize()
        .map_err(|error| format!("could not initialize POUNCE AC OPF: {error}"))?;
    let status = application.optimize_tnlp(Rc::clone(&tnlp) as Rc<RefCell<dyn TNLP>>);
    let statistics = application.statistics();
    if !matches!(
        status,
        ApplicationReturnStatus::SolveSucceeded | ApplicationReturnStatus::SolvedToAcceptableLevel
    ) {
        let qualification = if status == ApplicationReturnStatus::InfeasibleProblemDetected {
            "; this local NLP report is not a proof that the canonical problem is infeasible"
        } else {
            ""
        };
        return Err(format!(
            "POUNCE AC OPF stopped with {status:?} after {} iterations (constraint violation {:.3e}, KKT error {:.3e}){qualification}",
            statistics.iteration_count,
            statistics.final_unscaled_constr_viol,
            statistics.final_unscaled_kkt_error,
        ));
    }
    let solved = tnlp.borrow();
    let x = solved
        .final_x()
        .ok_or("POUNCE reported AC OPF convergence without a primal point")?;
    validate_primal(
        &model,
        x,
        solved.final_obj(),
        statistics.iteration_count,
        status,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use pounce_rs::{SparsityRequest, TNLP};
    use powerio::{BusId, LoadVoltageModel, Storage};
    use powerio_prob::{ActiveConstraints, ConstraintSelection, Objective};

    use super::*;
    use crate::model::{parse_matpower, CASE3};

    fn case3_model() -> CanonicalAcOpfModel {
        let network = parse_matpower(CASE3).expect("parse case3");
        let instance = AcOpfInstance::from_network(network).expect("case3 AC OPF instance");
        compile_ac_opf_model(&instance).expect("compile case3 AC OPF")
    }

    const CASE3_FULL_PI: &str = "\
function mpc = case3fullpi
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3  0  0  1.5  3.0 1 1.02  2 230 1 1.1 0.9;
 2 1 70 25  0.0 -2.0 1 0.98 -3 230 1 1.1 0.9;
 3 2 15  5  0.5  0.0 1 1.01  1 230 1 1.1 0.9;
];
mpc.gen = [
 1 55  8 100 -80 1.02 100 1 150  5 0 0 0 0 0 0 0 0 0 0 0;
 1 20 -3  60 -40 1.01 100 1  80  0 0 0 0 0 0 0 0 0 0 0;
 3 25  4  70 -50 1.01 100 1  90  0 0 0 0 0 0 0 0 0 0 0;
 2 99 99 100 -90 1.00 100 0 120 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.02 0.08 0.04  90 90 90 1.05  11 1 -35 30;
 2 3 0.01 0.06 0.03  75 75 75 0.97  -7 1 -25 40;
 3 1 0.03 0.09 0.02  65 65 65 0     0 1 -20 20;
 1 3 0.02 0.10 0.01 100 100 100 0   0 0 -30 30;
];
mpc.gencost = [
 2 0 0 3 0.10 2.0 10;
 2 0 0 3 0.12 1.5  5;
 2 0 0 3 0.08 2.5  2;
 2 0 0 3 0.50 9.0 99;
];
";

    fn full_pi_model() -> CanonicalAcOpfModel {
        let network = parse_matpower(CASE3_FULL_PI).expect("parse full pi case");
        let instance = AcOpfInstance::from_network(network).expect("full pi AC OPF instance");
        compile_ac_opf_model(&instance).expect("compile full pi AC OPF")
    }

    fn direct_branch_flow(prep: &AcOpfPreparation, x: &[f64], branch: usize) -> [f64; 4] {
        let n = prep.n_buses;
        let f = prep.branches.from_bus[branch];
        let t = prep.branches.to_bus[branch];
        let vf = x[n + f];
        let vt = x[n + t];
        let delta = x[f] - x[t];
        let wr = vf * vt * delta.cos();
        let wi = vf * vt * delta.sin();
        let g = prep.branches.g[branch];
        let b = prep.branches.b[branch];
        let tap = prep.branches.tap[branch];
        let tr = tap * prep.branches.shift[branch].cos();
        let ti = tap * prep.branches.shift[branch].sin();
        let tm2 = tap * tap;
        let pf = (g + prep.branches.g_fr[branch]) / tm2 * vf * vf
            + (-g * tr + b * ti) / tm2 * wr
            + (-b * tr - g * ti) / tm2 * wi;
        let qf = -(b + prep.branches.b_fr[branch]) / tm2 * vf * vf
            + (b * tr + g * ti) / tm2 * wr
            + (-g * tr + b * ti) / tm2 * wi;
        let pt = (g + prep.branches.g_to[branch]) * vt * vt
            + (-g * tr - b * ti) / tm2 * wr
            + (b * tr - g * ti) / tm2 * wi;
        let qt = -(b + prep.branches.b_to[branch]) * vt * vt
            + (b * tr - g * ti) / tm2 * wr
            + (g * tr + b * ti) / tm2 * wi;
        [pf, qf, pt, qt]
    }

    fn evaluate_constraints(model: &CanonicalAcOpfModel, x: &[f64]) -> Vec<f64> {
        let mut tnlp = model.tnlp().expect("build TNLP");
        let mut values = vec![0.0; model.problem().m];
        assert!(tnlp.eval_g(x, true, &mut values));
        values
    }

    fn jacobian_structure(tnlp: &mut NlTnlp) -> (Vec<i32>, Vec<i32>) {
        let nnz = usize::try_from(tnlp.get_nlp_info().expect("NLP dimensions").nnz_jac_g)
            .expect("nonnegative Jacobian nnz");
        let mut rows = vec![0; nnz];
        let mut columns = vec![0; nnz];
        assert!(tnlp.eval_jac_g(
            None,
            false,
            SparsityRequest::Structure {
                irow: &mut rows,
                jcol: &mut columns,
            },
        ));
        (rows, columns)
    }

    fn lagrangian_gradient(
        tnlp: &mut NlTnlp,
        x: &[f64],
        objective_factor: f64,
        lambda: &[f64],
        jac_rows: &[i32],
        jac_columns: &[i32],
    ) -> Vec<f64> {
        let mut gradient = vec![0.0; x.len()];
        assert!(tnlp.eval_grad_f(x, true, &mut gradient));
        for value in &mut gradient {
            *value *= objective_factor;
        }
        let mut jacobian = vec![0.0; jac_rows.len()];
        assert!(tnlp.eval_jac_g(
            Some(x),
            true,
            SparsityRequest::Values {
                values: &mut jacobian,
            },
        ));
        for ((&row, &column), &value) in jac_rows.iter().zip(jac_columns).zip(&jacobian) {
            gradient[usize::try_from(column).expect("Jacobian column")] +=
                lambda[usize::try_from(row).expect("Jacobian row")] * value;
        }
        gradient
    }

    fn directional_derivative_errors(model: &CanonicalAcOpfModel, tnlp: &mut NlTnlp) -> (f64, f64) {
        let n = model.problem().n;
        let m = model.problem().m;
        let x = &model.problem().x0;
        let direction = (0..n)
            .map(|column| ((column * 37 % 101) as f64 - 50.0) / 50.0)
            .collect::<Vec<_>>();
        let step = 1.0e-6;
        let plus = x
            .iter()
            .zip(&direction)
            .map(|(&value, &delta)| value + step * delta)
            .collect::<Vec<_>>();
        let minus = x
            .iter()
            .zip(&direction)
            .map(|(&value, &delta)| value - step * delta)
            .collect::<Vec<_>>();

        let (jac_rows, jac_columns) = jacobian_structure(tnlp);
        let mut jacobian = vec![0.0; jac_rows.len()];
        assert!(tnlp.eval_jac_g(
            Some(x),
            true,
            SparsityRequest::Values {
                values: &mut jacobian,
            },
        ));
        let mut exact_constraint_direction = vec![0.0; m];
        for ((&row, &column), &value) in jac_rows.iter().zip(&jac_columns).zip(&jacobian) {
            exact_constraint_direction[usize::try_from(row).expect("Jacobian row")] +=
                value * direction[usize::try_from(column).expect("Jacobian column")];
        }
        let mut plus_constraints = vec![0.0; m];
        let mut minus_constraints = vec![0.0; m];
        assert!(tnlp.eval_g(&plus, true, &mut plus_constraints));
        assert!(tnlp.eval_g(&minus, true, &mut minus_constraints));
        let jacobian_error = exact_constraint_direction
            .iter()
            .zip(plus_constraints.iter().zip(&minus_constraints))
            .map(|(&exact, (&plus, &minus))| (exact - (plus - minus) / (2.0 * step)).abs())
            .fold(0.0, f64::max);

        let info = tnlp.get_nlp_info().expect("NLP dimensions");
        let nnz_h = usize::try_from(info.nnz_h_lag).expect("nonnegative Hessian nnz");
        let mut h_rows = vec![0; nnz_h];
        let mut h_columns = vec![0; nnz_h];
        assert!(tnlp.eval_h(
            None,
            false,
            1.0,
            None,
            false,
            SparsityRequest::Structure {
                irow: &mut h_rows,
                jcol: &mut h_columns,
            },
        ));
        let lambda = (0..m)
            .map(|row| 0.001 + (row * 17 % 31) as f64 * 0.0001)
            .collect::<Vec<_>>();
        let objective_factor = 0.7;
        let mut hessian = vec![0.0; nnz_h];
        assert!(tnlp.eval_h(
            Some(x),
            true,
            objective_factor,
            Some(&lambda),
            true,
            SparsityRequest::Values {
                values: &mut hessian,
            },
        ));
        let mut exact_lagrangian_direction = vec![0.0; n];
        for ((&row, &column), &value) in h_rows.iter().zip(&h_columns).zip(&hessian) {
            let row = usize::try_from(row).expect("Hessian row");
            let column = usize::try_from(column).expect("Hessian column");
            exact_lagrangian_direction[row] += value * direction[column];
            if row != column {
                exact_lagrangian_direction[column] += value * direction[row];
            }
        }
        let plus_gradient = lagrangian_gradient(
            tnlp,
            &plus,
            objective_factor,
            &lambda,
            &jac_rows,
            &jac_columns,
        );
        let minus_gradient = lagrangian_gradient(
            tnlp,
            &minus,
            objective_factor,
            &lambda,
            &jac_rows,
            &jac_columns,
        );
        let hessian_error = exact_lagrangian_direction
            .iter()
            .zip(plus_gradient.iter().zip(&minus_gradient))
            .map(|(&exact, (&plus, &minus))| (exact - (plus - minus) / (2.0 * step)).abs())
            .fold(0.0, f64::max);
        (jacobian_error, hessian_error)
    }

    #[test]
    fn canonical_policy_and_layout_are_stable() {
        let first = case3_model();
        let second = case3_model();
        assert_eq!(first.policy, AcOpfAssemblyPolicy::CANONICAL);
        assert_eq!(first.preparation.units, Units::PerUnit);
        assert!(!first.preparation.skip_zero_impedance);
        assert!(!first.preparation.synthesize_unrated_limits);
        assert!(first.preparation.correct_angle_difference_bounds);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.columns.va, vec![0, 1, 2]);
        assert_eq!(first.columns.vm, vec![3, 4, 5]);
        assert_eq!(first.columns.pg, vec![6, 7]);
        assert_eq!(first.columns.qg, vec![8, 9]);
        assert_eq!(first.rows.p_balance, vec![0, 1, 2]);
        assert_eq!(first.rows.q_balance, vec![3, 4, 5]);
        assert_eq!(first.rows.reference_angle, vec![6]);
        assert_eq!(first.problem().n, 10);
        assert_eq!(first.problem().m, 16);
        assert!(first
            .problem()
            .x0
            .iter()
            .zip(&first.problem().x_l)
            .zip(&first.problem().x_u)
            .all(|((&start, &lower), &upper)| start.is_finite()
                && start >= lower
                && start <= upper));
    }

    #[test]
    fn pounce_solution_passes_independent_full_pi_validation() {
        let network = parse_matpower(CASE3_FULL_PI).expect("parse full pi case");
        let instance = AcOpfInstance::from_network(network).expect("full pi AC OPF instance");
        let solved = solve_ac_opf(&instance).expect("solve full pi AC OPF");
        assert!(matches!(
            solved.solver_status,
            ApplicationReturnStatus::SolveSucceeded
                | ApplicationReturnStatus::SolvedToAcceptableLevel
        ));
        assert!(solved.iterations > 0);
        assert!(solved.objective.is_finite());
        assert!(solved.residuals.max_violation() < PRIMAL_TOLERANCE);
        assert_eq!(solved.fingerprint.len(), 64);
    }

    #[test]
    fn full_pi_constraints_match_independent_equations() {
        let model = full_pi_model();
        let prep = &model.preparation;
        assert_eq!(prep.n_source_generators, 4);
        assert_eq!(prep.n_generators(), 3);
        assert_eq!(prep.n_source_branches, 4);
        assert_eq!(prep.n_branches(), 3);
        assert_eq!(prep.generators.source_rows, vec![Some(0), Some(1), Some(2)]);
        assert!(prep.branches.analysis_sources.len() == 3);
        assert!(prep
            .branches
            .tap
            .iter()
            .any(|tap| (*tap - 1.0).abs() > 1.0e-12));
        assert!(prep
            .branches
            .shift
            .iter()
            .any(|shift| shift.abs() > 1.0e-12));
        assert!(prep
            .branches
            .b_fr
            .iter()
            .any(|charging| charging.abs() > 0.0));
        assert!(prep.buses.g_s.iter().any(|shunt| shunt.abs() > 0.0));
        assert!(prep.buses.b_s.iter().any(|shunt| shunt.abs() > 0.0));

        let mut x = model.problem().x0.clone();
        x[model.columns.va[0]] = 0.08;
        x[model.columns.va[1]] = -0.06;
        x[model.columns.va[2]] = 0.03;
        x[model.columns.vm[0]] = 1.04;
        x[model.columns.vm[1]] = 0.96;
        x[model.columns.vm[2]] = 1.01;
        x[model.columns.pg[0]] = 0.60;
        x[model.columns.pg[1]] = 0.18;
        x[model.columns.pg[2]] = 0.24;
        x[model.columns.qg[0]] = 0.09;
        x[model.columns.qg[1]] = -0.02;
        x[model.columns.qg[2]] = 0.06;

        let values = evaluate_constraints(&model, &x);
        let flows = (0..prep.n_branches())
            .map(|branch| direct_branch_flow(prep, &x, branch))
            .collect::<Vec<_>>();
        let mut p = (0..prep.n_buses)
            .map(|bus| {
                -prep.buses.p_d[bus] - prep.buses.g_s[bus] * x[model.columns.vm[bus]].powi(2)
            })
            .collect::<Vec<_>>();
        let mut q = (0..prep.n_buses)
            .map(|bus| {
                -prep.buses.q_d[bus] + prep.buses.b_s[bus] * x[model.columns.vm[bus]].powi(2)
            })
            .collect::<Vec<_>>();
        for generator in 0..prep.n_generators() {
            let bus = prep.generators.bus_of_gen[generator];
            p[bus] += x[model.columns.pg[generator]];
            q[bus] += x[model.columns.qg[generator]];
        }
        for (branch, [pf, qf, pt, qt]) in flows.iter().copied().enumerate() {
            p[prep.branches.from_bus[branch]] -= pf;
            q[prep.branches.from_bus[branch]] -= qf;
            p[prep.branches.to_bus[branch]] -= pt;
            q[prep.branches.to_bus[branch]] -= qt;
        }
        for bus in 0..prep.n_buses {
            assert!((values[model.rows.p_balance[bus]] - p[bus]).abs() < 2.0e-12);
            assert!((values[model.rows.q_balance[bus]] - q[bus]).abs() < 2.0e-12);
        }
        for (position, &bus) in prep.reference_buses.iter().enumerate() {
            assert_eq!(
                values[model.rows.reference_angle[position]],
                x[model.columns.va[bus]]
            );
        }
        for (branch, &[pf, qf, pt, qt]) in flows.iter().enumerate() {
            let f = prep.branches.from_bus[branch];
            let t = prep.branches.to_bus[branch];
            if let Some(row) = model.rows.angle_difference[branch] {
                assert!(
                    (values[row] - (x[model.columns.va[f]] - x[model.columns.va[t]])).abs()
                        < 1.0e-14
                );
            }
            let limit_squared = prep.branches.s_max[branch].powi(2);
            if let Some(row) = model.rows.thermal_from[branch] {
                assert!((values[row] - (limit_squared - pf * pf - qf * qf)).abs() < 2.0e-12);
            }
            if let Some(row) = model.rows.thermal_to[branch] {
                assert!((values[row] - (limit_squared - pt * pt - qt * qt)).abs() < 2.0e-12);
            }
        }
    }

    #[test]
    fn exact_sparse_derivatives_match_finite_differences() {
        let model = full_pi_model();
        let mut tnlp = model.tnlp().expect("build TNLP");
        let info = tnlp.get_nlp_info().expect("NLP dimensions");
        let n = usize::try_from(info.n).expect("nonnegative variable count");
        let m = usize::try_from(info.m).expect("nonnegative constraint count");
        assert_eq!((n, m), (model.problem().n, model.problem().m));

        let x = model.problem().x0.clone();
        let mut analytic = vec![0.0; n];
        assert!(tnlp.eval_grad_f(&x, true, &mut analytic));
        let step = 1.0e-6;
        for column in 0..n {
            let mut plus = x.clone();
            let mut minus = x.clone();
            plus[column] += step;
            minus[column] -= step;
            let finite = (tnlp.eval_f(&plus, true).expect("f+")
                - tnlp.eval_f(&minus, true).expect("f-"))
                / (2.0 * step);
            assert!(
                (analytic[column] - finite).abs() < 2.0e-5,
                "objective derivative {column}: analytic={} finite={finite}",
                analytic[column]
            );
        }

        let (jac_rows, jac_columns) = jacobian_structure(&mut tnlp);
        let mut jacobian = vec![0.0; jac_rows.len()];
        assert!(tnlp.eval_jac_g(
            Some(&x),
            true,
            SparsityRequest::Values {
                values: &mut jacobian,
            },
        ));
        for ((&row, &column), &analytic_value) in jac_rows.iter().zip(&jac_columns).zip(&jacobian) {
            let row = usize::try_from(row).expect("Jacobian row");
            let column = usize::try_from(column).expect("Jacobian column");
            let mut plus = x.clone();
            let mut minus = x.clone();
            plus[column] += step;
            minus[column] -= step;
            let plus_values = evaluate_constraints(&model, &plus);
            let minus_values = evaluate_constraints(&model, &minus);
            let finite = (plus_values[row] - minus_values[row]) / (2.0 * step);
            assert!(
                (analytic_value - finite).abs() < 2.0e-5,
                "Jacobian ({row}, {column}): analytic={analytic_value} finite={finite}"
            );
        }

        let nnz_h = usize::try_from(info.nnz_h_lag).expect("nonnegative Hessian nnz");
        let mut h_rows = vec![0; nnz_h];
        let mut h_columns = vec![0; nnz_h];
        assert!(tnlp.eval_h(
            None,
            false,
            1.0,
            None,
            false,
            SparsityRequest::Structure {
                irow: &mut h_rows,
                jcol: &mut h_columns,
            },
        ));
        let lambda = (0..m)
            .map(|row| 0.05 + row as f64 * 0.013)
            .collect::<Vec<_>>();
        let objective_factor = 0.7;
        let mut hessian = vec![0.0; nnz_h];
        assert!(tnlp.eval_h(
            Some(&x),
            true,
            objective_factor,
            Some(&lambda),
            true,
            SparsityRequest::Values {
                values: &mut hessian,
            },
        ));
        let hessian_step = 2.0e-5;
        for ((&row, &column), &analytic_value) in h_rows.iter().zip(&h_columns).zip(&hessian) {
            let row = usize::try_from(row).expect("Hessian row");
            let column = usize::try_from(column).expect("Hessian column");
            let mut plus = x.clone();
            let mut minus = x.clone();
            plus[column] += hessian_step;
            minus[column] -= hessian_step;
            let plus_gradient = lagrangian_gradient(
                &mut tnlp,
                &plus,
                objective_factor,
                &lambda,
                &jac_rows,
                &jac_columns,
            );
            let minus_gradient = lagrangian_gradient(
                &mut tnlp,
                &minus,
                objective_factor,
                &lambda,
                &jac_rows,
                &jac_columns,
            );
            let finite = (plus_gradient[row] - minus_gradient[row]) / (2.0 * hessian_step);
            let tolerance = 2.0e-4 * analytic_value.abs().max(1.0);
            assert!(
                (analytic_value - finite).abs() < tolerance,
                "Hessian ({row}, {column}): analytic={analytic_value} finite={finite} tolerance={tolerance}"
            );
        }
    }

    #[test]
    #[ignore = "set TELLEGEN_ACOPF_FIXTURES to a directory containing case14.m, case30.m, and case300.m"]
    fn external_model_build_ladder() {
        let directory = std::env::var_os("TELLEGEN_ACOPF_FIXTURES")
            .map(std::path::PathBuf::from)
            .expect("TELLEGEN_ACOPF_FIXTURES is required for the ignored build ladder");
        println!("case,buses,generators,branches,variables,constraints,nnz_jacobian,nnz_hessian,compile_ms,tape_ms,jacobian_direction_error,hessian_direction_error");
        for (filename, expected_buses) in [("case14.m", 14), ("case30.m", 30), ("case300.m", 300)] {
            let text = std::fs::read_to_string(directory.join(filename))
                .unwrap_or_else(|error| panic!("read {filename}: {error}"));
            let network =
                parse_matpower(&text).unwrap_or_else(|error| panic!("parse {filename}: {error}"));
            let instance = AcOpfInstance::from_network(network)
                .unwrap_or_else(|error| panic!("build {filename} instance: {error}"));
            let compile_started = Instant::now();
            let model = compile_ac_opf_model(&instance)
                .unwrap_or_else(|error| panic!("compile {filename}: {error}"));
            let compile_ms = compile_started.elapsed().as_secs_f64() * 1_000.0;
            let tape_started = Instant::now();
            let mut tnlp = model
                .tnlp()
                .unwrap_or_else(|error| panic!("build {filename} derivative tape: {error}"));
            let tape_ms = tape_started.elapsed().as_secs_f64() * 1_000.0;
            let info = tnlp
                .get_nlp_info()
                .unwrap_or_else(|| panic!("read {filename} NLP dimensions"));
            assert_eq!(model.preparation.n_buses, expected_buses);
            let (jacobian_error, hessian_error) = directional_derivative_errors(&model, &mut tnlp);
            assert!(
                jacobian_error < 2.0e-5,
                "{filename} Jacobian directional error {jacobian_error}"
            );
            assert!(
                hessian_error < 2.0e-4,
                "{filename} Hessian directional error {hessian_error}"
            );
            println!(
                "{filename},{},{},{},{},{},{},{},{compile_ms:.3},{tape_ms:.3},{jacobian_error:.3e},{hessian_error:.3e}",
                model.preparation.n_buses,
                model.preparation.n_generators(),
                model.preparation.n_branches(),
                info.n,
                info.m,
                info.nnz_jac_g,
                info.nnz_h_lag,
            );
        }
    }

    #[test]
    fn piecewise_cost_uses_an_exact_epigraph() {
        let text = CASE3.replacen("2 0 0 3 0.11  5   0;", "1 0 0 3 0 0 100 500 250 2000;", 1);
        let network = parse_matpower(&text).expect("parse piecewise case");
        let instance = AcOpfInstance::from_network(network).expect("piecewise AC OPF instance");
        let model = compile_ac_opf_model(&instance).expect("compile piecewise AC OPF");
        let epigraph = model.columns.piecewise_epigraph[0].expect("piecewise epigraph");
        assert_eq!(model.rows.piecewise_segments[0].len(), 2);
        assert_eq!(model.problem().n, 11);
        let x = &model.problem().x0;
        let constraints = evaluate_constraints(&model, x);
        assert!(model.rows.piecewise_segments[0]
            .iter()
            .all(|&row| constraints[row] >= -1.0e-12));
        let mut tnlp = model.tnlp().expect("build TNLP");
        let objective = tnlp.eval_f(x, true).expect("objective");
        let p0 = x[model.columns.pg[0]];
        let p1 = x[model.columns.pg[1]];
        let curve = model.preparation.generators.piecewise_linear[0]
            .as_ref()
            .expect("prepared piecewise curve");
        let expected_piecewise = (0..curve.power.len() - 1)
            .map(|segment| {
                let slope = (curve.value[segment + 1] - curve.value[segment])
                    / (curve.power[segment + 1] - curve.power[segment]);
                slope * p0 + curve.value[segment] - slope * curve.power[segment]
            })
            .fold(f64::NEG_INFINITY, f64::max);
        let expected_quadratic = 0.5 * model.preparation.generators.q[1] * p1 * p1
            + model.preparation.generators.c[1] * p1
            + model.preparation.generators.c0[1];
        assert!((x[epigraph] - expected_piecewise).abs() < 1.0e-12);
        assert!((objective - expected_piecewise - expected_quadratic).abs() < 1.0e-9);

        let feasibility = AcOpfInstance::from_network(
            parse_matpower(&text).expect("parse feasibility piecewise case"),
        )
        .expect("feasibility AC OPF instance")
        .with_objective(Objective::none());
        let feasibility = compile_ac_opf_model(&feasibility).expect("compile feasibility AC OPF");
        assert_eq!(feasibility.problem().n, 10);
        assert!(feasibility
            .columns
            .piecewise_epigraph
            .iter()
            .all(Option::is_none));
        assert!(feasibility
            .rows
            .piecewise_segments
            .iter()
            .all(Vec::is_empty));
        let mut tnlp = feasibility.tnlp().expect("build feasibility TNLP");
        assert_eq!(tnlp.eval_f(&feasibility.problem().x0, true), Some(0.0));
    }

    #[test]
    fn powerio_constraint_selections_control_bounds_and_rows() {
        let network = parse_matpower(CASE3_FULL_PI).expect("parse full pi case");
        let mut constraints = ActiveConstraints::default();
        constraints.generator_capability = ConstraintSelection::None;
        constraints.voltage_bounds = ConstraintSelection::None;
        constraints.thermal_limits = ConstraintSelection::None;
        constraints.angle_bounds = ConstraintSelection::None;
        let instance = AcOpfInstance::from_network(network)
            .expect("AC OPF instance")
            .with_constraints(constraints);
        let model = compile_ac_opf_model(&instance).expect("compile relaxed AC OPF");

        for &column in &model.columns.vm {
            assert_eq!(model.problem().x_l[column], 0.0);
            assert_eq!(model.problem().x_u[column], INF);
        }
        for &column in model.columns.pg.iter().chain(&model.columns.qg) {
            assert_eq!(model.problem().x_l[column], -INF);
            assert_eq!(model.problem().x_u[column], INF);
        }
        assert!(model.rows.angle_difference.iter().all(Option::is_none));
        assert!(model.rows.thermal_from.iter().all(Option::is_none));
        assert!(model.rows.thermal_to.iter().all(Option::is_none));
        assert_eq!(
            model.problem().m,
            2 * model.preparation.n_buses + model.preparation.reference_buses.iter().len()
        );
    }

    #[test]
    fn unsupported_features_name_the_source_element() {
        let mut network = parse_matpower(CASE3).expect("parse case3");
        network.loads_mut()[0].uid = Some("zip-load".into());
        network.loads_mut()[0].voltage_model = Some(LoadVoltageModel::Exponential {
            p: 90.0,
            q: 30.0,
            v_nom: Some(1.0),
            gamma_p: 1.0,
            gamma_q: 2.0,
        });
        let instance = AcOpfInstance::from_network(network).expect("ZIP AC OPF instance");
        let error = compile_ac_opf_model(&instance).expect_err("reject ZIP load");
        assert!(error.contains("zip-load") && error.contains("voltage-dependent"));

        let mut network = parse_matpower(CASE3).expect("parse case3");
        network.generators_mut()[0].uid = Some("remote-gen".into());
        network.generators_mut()[0].regulated_bus = Some(BusId(2));
        let instance = AcOpfInstance::from_network(network).expect("remote control instance");
        let error = compile_ac_opf_model(&instance).expect_err("reject remote control");
        assert!(error.contains("remote-gen") && error.contains("remote bus"));

        let mut network = parse_matpower(CASE3).expect("parse case3");
        let mut storage = Storage::new(BusId(2));
        storage.uid = Some("battery-2".into());
        network.storage_mut().push(storage);
        let instance = AcOpfInstance::from_network(network).expect("storage AC OPF instance");
        let error = compile_ac_opf_model(&instance).expect_err("reject storage");
        assert!(error.contains("battery-2") && error.contains("storage behavior"));
    }
}
