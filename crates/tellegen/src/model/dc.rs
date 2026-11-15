//! DC OPF model: the B-theta data — incidence, susceptances, generator bounds,
//! quadratic costs, and line flow limits — read from PowerIO's DC OPF
//! preparation of a `BalancedNetwork` or a typed `DcOpfInstance`. A network is
//! prepared with PowerIO's PowerModels angle difference pad, and an unrated
//! branch's limit is synthesized over the padded window.

use powerio::BalancedNetwork;
use powerio::BranchSusceptanceFormula;
use powerio_matrix::{
    build_dc_opf_preparation, calc_solver_branch_flow_matrix, calc_weighted_laplacian,
    AnalysisBranchSource, DcOpfAssemblyOptions, DcOpfPreparation, PreparedObjective, SparseMatrix,
    SynthesizedLimitWindow, Units,
};
use powerio_prob::DcOpfInstance;
use powerio_tx::{AngleDifferenceBounds, HvdcTreatment};

use crate::api::SolveDiagnostic;

use super::islands::{energize, ReferencePolicy};
use super::{
    dc_diagnostics, network_branch_ids, reject_unsupported_active_elements, source_position_ids,
    uids_for_source_rows, validate_canonical_identity, validate_model_network, HvdcLines,
    PiecewiseCost,
};

/// Load-shedding cost = multiplier x peak marginal generation cost, so the
/// solver only sheds when capacity or the network physically cannot serve the
/// load.
const DEFAULT_SHED_COST_MULTIPLIER: f64 = 10.0;

/// B-theta DC OPF model data. Indices are dense `[0, n)` / `[0, m)` / `[0, k)`
/// over PowerIO's preparation; `bus_ids`, `branch_ids`, and `gen_ids` map dense
/// indices back to source ids for output payloads.
///
/// Every DC operator comes from PowerIO's preparation unchanged: the signed
/// bus by branch incidence `A`, the susceptance weighted Laplacian
/// `L = A diag(-b) A'`, and the branch flow matrix `Bf = diag(-b) A'`. DC
/// power balance is `G_inc pg + psh - fixed_withdrawal = L theta`; branch
/// flows are `f = Bf theta + flow_offset`. A branch out of service never
/// reaches the preparation, so no operator carries a switching multiplier.
#[derive(Clone)]
// Every field is derived together by `from_network` from one PowerIO
// preparation: dense reindexing, per unit conversion, and the source row
// maps only agree with each other as a set. A hand-built literal that got one
// of them wrong would solve to a wrong answer rather than fail, so the
// constructor is the only supported way in and new fields can arrive without a
// major bump.
#[non_exhaustive]
pub(crate) struct DcNetwork {
    pub formula: crate::DcFormula,
    /// d(effective signed DC coefficient)/d(raw series susceptance), holding g
    /// fixed: PowerIO's
    /// `BranchSusceptanceFormula::calc_susceptance_sensitivity_to_series_susceptance`.
    #[cfg(feature = "sensitivity")]
    pub db_dseries_b: Vec<f64>,
    /// Buses, branches, generators after filtering (in-service, non-isolated).
    pub n: usize,
    pub m: usize,
    pub k: usize,
    /// PowerIO's signed bus by branch incidence `A` (`n x m` CSR,
    /// `DcOpfPreparation::calc_bus_branch_incidence`): row `i` lists the
    /// `(branch, +-1)` pairs at bus `i`, `+1` where the branch leaves.
    pub(crate) incidence: SparseMatrix,
    /// The same incidence by branch, `A'` (`m x n` CSR): row `e` holds `+1` at
    /// the branch's from bus and `-1` at its to bus.
    pub(crate) branch_incidence: SparseMatrix,
    /// PowerIO's DC bus susceptance matrix `L = A diag(-b) A'` (`n x n` CSR,
    /// `calc_weighted_laplacian` over the preparation's positive weights).
    pub(crate) laplacian: SparseMatrix,
    /// PowerIO's branch flow matrix `Bf = diag(-b) A'` (`m x n` CSR,
    /// `calc_solver_branch_flow_matrix`).
    pub(crate) branch_flow_matrix: SparseMatrix,
    /// Bus each generator injects at, dense index.
    pub gen_bus: Vec<usize>,
    /// Branch susceptance `b = -x / (r^2 + x^2)` (negative for inductive
    /// branches). A branch with tiny but nonzero impedance — a
    /// substation bus-splitting jumper, common in detailed synthetic cases —
    /// gets a correspondingly large `|b|` rather than being dropped; it is a
    /// real, near-ideal tie, not an open circuit.
    #[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
    pub b: Vec<f64>,
    /// Branch phase shifts in radians, in the active branch order from the
    /// powerio problem instance.
    #[cfg(feature = "sensitivity")]
    pub shift: Vec<f64>,
    /// Fixed affine branch flow term `-b_powerio .* shift`, PowerIO's
    /// `calc_branch_flow_offset` of [`Self::prep`].
    pub flow_offset: Vec<f64>,
    /// Per unit thermal limit per branch (`rate_a`, with a fallback synthesized
    /// over the padded angle window when the source leaves it at 0).
    pub fmax: Vec<f64>,
    /// Per-unit generator output bounds.
    pub gmax: Vec<f64>,
    pub gmin: Vec<f64>,
    /// Phase-angle-difference bounds per branch (radians).
    pub angmin: Vec<f64>,
    pub angmax: Vec<f64>,
    /// Per-unit quadratic, linear, and constant (no-load) generation cost
    /// coefficients: the cost of generator `i` is `cq[i] pg[i]^2 + cl[i] pg[i] +
    /// cc[i]`. `cc` does not enter the QP objective (a constant does not move the
    /// argmin), but the solve readout adds `sum(cc)` back onto the reported
    /// objective so it matches a reference OPF objective that includes it.
    pub cq: Vec<f64>,
    pub cl: Vec<f64>,
    pub cc: Vec<f64>,
    /// Exact convex piecewise linear costs aligned with the generator columns.
    /// A present curve replaces the zero entries in `cq`, `cl`, and `cc` for
    /// that generator and is formulated with an epigraph variable.
    pub(crate) piecewise_costs: Vec<Option<PiecewiseCost>>,
    /// Load-shedding penalty per bus.
    pub c_shed: Vec<f64>,
    /// PowerIO's DC OPF preparation the model reads its nodal quantities
    /// from: per unit demand `p_d` (edited in place by demand deltas), shunt
    /// conductance withdrawal `g_s`, phase shift withdrawal `p_shift`, and
    /// fixed HVDC injection `p_hvdc`, with the branch weights and shifts that
    /// define every flow. The fixed withdrawal, the branch flows, and the
    /// generator to bus map are its own calculations
    /// (`calc_fixed_nodal_withdrawal`, `calc_branch_flow`,
    /// `calc_generator_bus_incidence`).
    pub(crate) prep: DcOpfPreparation,
    /// Reference buses, dense indices: at least one in every island, each
    /// grounded at angle zero.
    pub ref_buses: Vec<usize>,
    /// Whether load shedding is permitted. Portable PowerIO instances do not state this
    /// relaxation, so constructed workspaces keep the shedding variables pinned to zero.
    /// Internal numerical tests opt in explicitly when exercising the shedding KKT.
    pub allow_shed: bool,
    /// The objective declared by the PowerIO problem instance.
    pub(crate) objective: PreparedObjective,
    /// Active constraint masks, aligned with the generator and branch columns.
    pub(crate) generator_capability_active: Vec<bool>,
    pub(crate) thermal_limit_active: Vec<bool>,
    pub(crate) angle_bound_active: Vec<bool>,
    /// Whether each branch's thermal limit comes from a stated rating rather
    /// than one synthesized for an unrated branch.
    pub(crate) thermal_rated: Vec<bool>,
    /// Dense bus index -> original source bus id.
    pub bus_ids: Vec<usize>,
    /// Dense bus column to the star-lowered analysis row and source bus row.
    /// Synthetic star buses have no source row.
    pub(crate) bus_analysis_rows: Vec<usize>,
    pub(crate) bus_source_rows: Vec<Option<usize>>,
    /// Dense branch index -> one based source branch position for Tellegen's
    /// legacy generic solve response.
    pub branch_ids: Vec<usize>,
    /// Dense branch index -> PowerIO's canonical branch identity (`uid` when
    /// present, otherwise `branches:{source_row}`). Synthetic lowering rows
    /// retain the identity emitted by the PowerIO preparation.
    #[cfg(feature = "sensitivity")]
    pub(crate) branch_identities: Vec<String>,
    /// Dense generator index -> one based source generator position for
    /// Tellegen's legacy generic solve response.
    pub gen_ids: Vec<usize>,
    /// Dense branch index -> original source branch row. `None` marks a branch
    /// synthesized while lowering a source element.
    pub branch_source_rows: Vec<Option<usize>>,
    /// Dense branch index -> the source component PowerIO lowered it from: a
    /// branch row, or one winding of a three winding transformer.
    pub(crate) branch_sources: Vec<AnalysisBranchSource>,
    /// Dense generator index -> original source generator row.
    pub gen_source_rows: Vec<Option<usize>>,
    /// Dense bus index -> powerio row uid (`None` when the source network carried
    /// no uids). Lets edits address a bus as `"buses:0"` and responses echo the uid.
    pub bus_uids: Vec<Option<String>>,
    /// Dense branch index -> powerio row uid; the branch counterpart of `bus_uids`.
    pub branch_uids: Vec<Option<String>>,
    /// Dense generator index -> powerio row uid; the generator counterpart of
    /// `bus_uids`.
    pub(crate) gen_uids: Vec<Option<String>>,
    /// Caller-supplied linear rows over the flow and dispatch columns, resolved
    /// from a request's `constraints`. Empty unless a request states some.
    pub(crate) linear_rows: Vec<LinearRow>,
    /// System base power (MVA), for recovering served units from per unit results.
    pub base_mva: f64,
    /// How the model treated the network's islands, reported with the solve.
    pub(crate) diagnostics: Vec<SolveDiagnostic>,
}

/// One caller-supplied linear constraint resolved onto the DC OPF columns:
/// `lower_mw <= base_mva * (sum(flow) + sum(generation)) <= upper_mw`, where
/// `flow` weights the per unit branch flow columns `f` and `generation` the per
/// unit dispatch columns `pg`. A bus injection term is already expanded onto the
/// flows of the branches at the bus, so no row depends on demand. Equal limits
/// state an equality.
#[derive(Clone, Debug)]
pub(crate) struct LinearRow {
    /// The caller's name for the row.
    pub(crate) id: String,
    /// Dense branch index and coefficient, sorted, nonzero.
    pub(crate) flow: Vec<(usize, f64)>,
    /// Dense generator index and coefficient, sorted, nonzero.
    pub(crate) generation: Vec<(usize, f64)>,
    /// Limits in MW, as stated. The assembly scales them to per unit.
    pub(crate) lower_mw: Option<f64>,
    pub(crate) upper_mw: Option<f64>,
}

impl LinearRow {
    /// Whether the row is an equality (`lower_mw == upper_mw`).
    pub(crate) fn is_equality(&self) -> bool {
        matches!((self.lower_mw, self.upper_mw), (Some(lower), Some(upper)) if lower == upper)
    }

    /// The row's value `sum(flow) + sum(generation)` at a flow and dispatch point.
    pub(crate) fn evaluate(&self, f: &[f64], pg: &[f64]) -> f64 {
        let flow: f64 = self.flow.iter().map(|&(e, c)| c * f[e]).sum();
        let generation: f64 = self.generation.iter().map(|&(j, c)| c * pg[j]).sum();
        flow + generation
    }
}

impl DcNetwork {
    /// Build the DC OPF model from a parsed powerio `BalancedNetwork`.
    ///
    /// Builds a `powerio-prob` [`DcOpfInstance`](powerio_prob::DcOpfInstance)
    /// over the network as given and reads PowerIO's preparation of it, the
    /// owner of the nodal and generator interpretation (per unit withdrawal,
    /// branch phase terms, generator PQ bounds, source rows, synthesized thermal
    /// limits, and reference coverage). The angle difference bounds are
    /// PowerIO's PowerModels pad, and an unrated branch, a lowered winding
    /// included, takes the limit PowerIO synthesizes over the padded window.
    /// PowerIO preserves quadratic and piecewise linear costs as distinct
    /// preparation columns.
    #[cfg(test)]
    pub fn from_network(raw: &BalancedNetwork) -> Result<DcNetwork, String> {
        Self::from_network_with_formula(raw, BranchSusceptanceFormula::default())
    }

    pub fn from_network_with_formula(
        raw: &BalancedNetwork,
        formula: BranchSusceptanceFormula,
    ) -> Result<DcNetwork, String> {
        // A network's references count only where they host an in-service
        // generator, the rule PowerIO normalization applies.
        let energized = energize(raw, ReferencePolicy::GeneratorHosted);
        let mut model = Self::build(energized.network(raw), formula)?;
        let mut diagnostics = energized.diagnostics;
        diagnostics.append(&mut model.diagnostics);
        model.diagnostics = diagnostics;
        Ok(model)
    }

    /// Build the private solver workspace from a typed PowerIO
    /// [`DcOpfInstance`]. PowerIO compiles the instance's objective and active
    /// constraint selections. Unsupported terms and unknown selected identities
    /// return its typed preparation error instead of becoming a default problem.
    pub fn from_instance(instance: &DcOpfInstance) -> Result<DcNetwork, String> {
        validate_canonical_identity(instance.network())?;
        let energized = energize(instance.network(), ReferencePolicy::Stated);
        let diagnostics = energized.diagnostics.clone();
        let adjusted = energized
            .into_network()
            .map(|network| instance.clone().with_network(network))
            .transpose()
            .map_err(|e| e.to_string())?;
        let instance = adjusted.as_ref().unwrap_or(instance);
        reject_unsupported_active_elements(instance.network(), HvdcLines::FixedInjection)?;
        let mut model = Self::build_prepared(instance, instance.network(), false)?;
        let mut diagnostics = diagnostics;
        diagnostics.append(&mut model.diagnostics);
        model.diagnostics = diagnostics;
        Ok(model)
    }

    fn build(
        raw: &BalancedNetwork,
        formula: BranchSusceptanceFormula,
    ) -> Result<DcNetwork, String> {
        validate_model_network(raw, HvdcLines::FixedInjection)?;
        // The typed instance owns the case interpretation; the preparation is
        // PowerIO's one numerical assembly (per unit generator PQ bounds and
        // costs, nodal demand, reference coverage, synthesized thermal
        // limits, source rows), read straight from the caller's network.
        let instance = DcOpfInstance::from_network(raw.clone())
            .map_err(|e| e.to_string())?
            .with_branch_susceptance_formula(formula);
        Self::build_prepared(&instance, raw, true)
    }

    /// `from_network` is set when the model comes from a caller's network
    /// rather than a typed instance: the angle difference bounds are then
    /// PowerIO's PowerModels pad, unrated limits span the padded window, and
    /// lowered windings keep Tellegen's legacy branch numbering.
    fn build_prepared(
        instance: &DcOpfInstance,
        raw: &BalancedNetwork,
        from_network: bool,
    ) -> Result<DcNetwork, String> {
        let mut assembly = DcOpfAssemblyOptions::default()
            .with_units(Units::PerUnit)
            .with_skip_zero_impedance(false)
            .with_synthesize_unrated_limits(true)
            .with_hvdc_treatment(HvdcTreatment::FixedInjection);
        if from_network {
            assembly = assembly
                .with_angle_difference_bounds(AngleDifferenceBounds::PowerModelsPad)
                .with_synthesized_limit_window(SynthesizedLimitWindow::SelectedBounds);
        }
        let mut prep = build_dc_opf_preparation(instance, &assembly)
            .map_err(|e| format!("{}: {e}", e.code().code))?;
        // A preparation document written before PowerIO 0.11.7 carries no
        // `p_hvdc`, which PowerIO reads as zero; keep one entry per bus.
        if prep.p_hvdc.is_empty() {
            prep.p_hvdc = vec![0.0; prep.n_buses];
        }
        // The analysis sources index the network the preparation read.
        let prepared = instance.network();
        let thermal_rated = prep
            .branches
            .analysis_sources
            .iter()
            .map(|source| match *source {
                AnalysisBranchSource::Branch { row } => {
                    !crate::ratings::unlimited(prepared.branches()[row].rate_a)
                }
                AnalysisBranchSource::ThreeWindingTransformerWinding {
                    transformer_row,
                    winding,
                } => !crate::ratings::unlimited(
                    prepared.transformers_3w()[transformer_row].windings[winding].rate_a,
                ),
                // A source kind this build does not know keeps its limit.
                _ => true,
            })
            .collect();

        let n = prep.n_buses;
        let m = prep.n_branches();
        let k = prep.n_generators();

        // PowerIO aligned every source row with the dense preparation, over
        // the network it read: follow that map instead of rebuilding a bus
        // index or scanning raw IDs to interpret the prepared columns.
        let bus_ids: Vec<usize> = prep.bus_ids.iter().map(|id| id.0).collect();
        let bus_uids =
            uids_for_source_rows(&prep.bus_source_rows, raw.buses(), |bus| &bus.uid, "bus")?;
        let bus_source_rows = prep.bus_source_rows.clone();
        let branch_source_rows = super::branch_source_rows(&prep.branches.analysis_sources);
        let gen_source_rows = prep.generators.source_rows.clone();
        let branch_ids = if from_network {
            network_branch_ids(&prep.branches.analysis_sources, raw)?
        } else {
            source_position_ids(&branch_source_rows, prep.n_source_branches, "branch")?
        };
        let gen_ids = source_position_ids(&gen_source_rows, prep.n_source_generators, "generator")?;
        let branch_uids = uids_for_source_rows(
            &branch_source_rows,
            raw.branches(),
            |branch| &branch.uid,
            "branch",
        )?;
        let gen_uids = uids_for_source_rows(
            &gen_source_rows,
            raw.generators(),
            |generator| &generator.uid,
            "generator",
        )?;
        // PowerIO's canonical identity aligned with every prepared branch
        // column: `uid` when present, otherwise `branches:{source_row}`.
        #[cfg(feature = "sensitivity")]
        let branch_identities = prep.branches.identities.clone();

        // Per-bus demand and references, moved straight out of the freshly built,
        // locally owned instance: from_network runs per Study commit and preview, so
        // this stays clone free. PowerIO has checked that every island holds a
        // reference; the program grounds each one.
        let ref_buses: Vec<usize> = prep.reference_buses.iter().copied().collect();
        let flow_offset = prep.calc_branch_flow_offset();

        // The preparation carries positive solver edge weights; tellegen
        // stores the PowerModels-signed susceptance (negative for an
        // inductive branch), so every branch column and affine term negates
        // out of the same assembly. The relation is pinned upstream by
        // powerio-matrix's cross assembly conformance test.
        let operators = ModelOperators::of(
            prep.calc_bus_branch_incidence(),
            &prep.branches.susceptance_magnitude,
        );
        let b = prep
            .branches
            .susceptance_magnitude
            .iter()
            .map(|value| -value)
            .collect();
        #[cfg(feature = "sensitivity")]
        let shift = prep.branches.shift.clone();
        let angmin = prep.branches.angle_min.clone();
        let angmax = prep.branches.angle_max.clone();
        // PowerIO synthesizes missing limits for both source and star-lowered
        // branches before returning the aligned preparation.
        let fmax = prep.branches.f_max.clone();

        // powerio states the objective as `0.5*q*p^2 + c*p + c0`; Tellegen's
        // stored `cq` is the coefficient of `p^2` before its solver writes `2*cq`
        // on the Hessian.
        let gen_bus = prep.generators.bus_of_gen.clone();
        let cq: Vec<f64> = prep.generators.q.iter().map(|value| value / 2.0).collect();
        let cl = prep.generators.c.clone();
        let cc = prep.generators.c0.clone();
        let piecewise_costs = prep
            .generators
            .piecewise_linear
            .clone()
            .into_iter()
            .map(|cost| cost.map(PiecewiseCost::from_prepared))
            .collect::<Vec<_>>();
        let gmax = prep.generators.pmax.clone();
        let gmin = prep.generators.pmin.clone();

        // Shedding cost references the steepest marginal generation cost. Generator
        // bounds and cost columns are not checked for finiteness anywhere upstream,
        // and a single non-finite term would either hand the solver a non-finite
        // objective or (through `0.0 * inf` = NaN, which `f64::max` ignores) collapse
        // the shed price to the floor and quietly shed load that should be served.
        let marginal_cost_ub = (0..k)
            .map(|i| {
                piecewise_costs[i]
                    .as_ref()
                    .map_or(2.0 * cq[i] * gmax[i] + cl[i], PiecewiseCost::maximum_slope)
            })
            .filter(|value| value.is_finite())
            .fold(f64::NEG_INFINITY, f64::max)
            .max(1.0);
        let c_shed = vec![DEFAULT_SHED_COST_MULTIPLIER * marginal_cost_ub; n];

        let formula = crate::DcFormula::try_from(instance.branch_susceptance_formula())?;
        #[cfg(feature = "sensitivity")]
        let db_dseries_b = {
            // The analysis rows index PowerIO's star-lowered view, which holds
            // the lowered winding branches as well as the source rows.
            let view = powerio_tx::IndexedNetwork::new(instance.network());
            let formula = instance.branch_susceptance_formula();
            prep.branches
                .analysis_rows
                .iter()
                .map(|&row| {
                    let branch = &view.network().branches()[row];
                    formula.calc_susceptance_sensitivity_to_series_susceptance(
                        branch.r,
                        branch.x,
                        branch.calc_effective_tap(),
                    )
                })
                .collect()
        };
        Ok(DcNetwork {
            formula,
            #[cfg(feature = "sensitivity")]
            db_dseries_b,
            n,
            m,
            k,
            incidence: operators.incidence,
            branch_incidence: operators.branch_incidence,
            laplacian: operators.laplacian,
            branch_flow_matrix: operators.branch_flow_matrix,
            gen_bus,
            b,
            #[cfg(feature = "sensitivity")]
            shift,
            flow_offset,
            fmax,
            gmax,
            gmin,
            angmin,
            angmax,
            cq,
            cl,
            cc,
            piecewise_costs,
            c_shed,
            ref_buses,
            allow_shed: false,
            objective: prep.objective,
            generator_capability_active: prep.generators.capability_active.clone(),
            thermal_limit_active: prep.branches.thermal_limit_active.clone(),
            thermal_rated,
            angle_bound_active: prep.branches.angle_bound_active.clone(),
            bus_ids,
            bus_analysis_rows: prep.bus_analysis_rows.clone(),
            bus_source_rows,
            branch_ids,
            #[cfg(feature = "sensitivity")]
            branch_identities,
            gen_ids,
            branch_source_rows,
            branch_sources: prep.branches.analysis_sources.clone(),
            gen_source_rows,
            bus_uids,
            branch_uids,
            gen_uids,
            linear_rows: Vec::new(),
            base_mva: prep.base_mva,
            prep,
            // `raw` is the caller's network after energizing, in its own units,
            // so the transfer totals read in MW and an HVDC line whose terminal
            // sits in a de-energized island is reported as left out.
            diagnostics: dc_diagnostics(raw),
        })
    }

    /// Fixed nodal withdrawal at the current demand,
    /// `p_d + g_s + p_shift - p_hvdc`: PowerIO's `calc_fixed_nodal_withdrawal`
    /// over the (possibly edited) preparation.
    pub fn current_fixed_withdrawal(&self) -> Vec<f64> {
        self.prep.calc_fixed_nodal_withdrawal()
    }

    /// Branch flows at the bus angles `theta`, `f = Bf theta + flow_offset`:
    /// PowerIO's `calc_branch_flow` over the preparation.
    #[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
    pub(crate) fn branch_flow(&self, theta: &[f64]) -> Vec<f64> {
        self.prep.calc_branch_flow(theta)
    }

    /// The shedding upper bound at bus `i`: the curtailable load `max(d, 0)` when
    /// shedding is permitted, else `0` (the variable is pinned to zero). The one source
    /// of this rule — the DC OPF solve bounds `psh` by it, and the KKT sensitivity
    /// derivation reads the same cap so its linearization matches the solved program.
    pub fn shed_cap(&self, i: usize) -> f64 {
        if self.allow_shed {
            self.prep.p_d[i].max(0.0)
        } else {
            0.0
        }
    }

    /// Evaluate one declared generator cost at `power` in per unit.
    pub(crate) fn generator_cost(&self, generator: usize, power: f64) -> f64 {
        self.piecewise_costs[generator].as_ref().map_or_else(
            || self.cq[generator] * power * power + self.cl[generator] * power + self.cc[generator],
            |cost| cost.evaluate(power),
        )
    }

    /// The angle difference `(A' theta)[e]` across branch `e`, read from
    /// PowerIO's incidence.
    #[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
    pub(crate) fn angle_difference(&self, theta: &[f64], e: usize) -> f64 {
        csr_row(&self.branch_incidence, e)
            .map(|(bus, sign)| sign * theta[bus])
            .sum()
    }

    /// Rebuild every weighted quantity after a numerical test perturbs `b`:
    /// the preparation's weights take the perturbed values, and the operators,
    /// the flow offset, and the nodal phase shift withdrawal come from the
    /// same PowerIO calls the constructor makes.
    #[cfg(test)]
    pub(crate) fn reweight(&mut self) {
        self.prep.branches.susceptance_magnitude = self.b.iter().map(|b| -b).collect();
        let operators = ModelOperators::of(
            self.incidence.clone(),
            &self.prep.branches.susceptance_magnitude,
        );
        self.laplacian = operators.laplacian;
        self.branch_flow_matrix = operators.branch_flow_matrix;
        self.flow_offset = self.prep.calc_branch_flow_offset();
        self.prep.p_shift = self.prep.calc_phase_shift_withdrawal();
    }
}

/// The DC operators a model reads, every one PowerIO's own: the incidence of
/// a preparation and the weighted matrices `calc_weighted_laplacian` and
/// `calc_solver_branch_flow_matrix` build over the positive weights.
struct ModelOperators {
    incidence: SparseMatrix,
    branch_incidence: SparseMatrix,
    laplacian: SparseMatrix,
    branch_flow_matrix: SparseMatrix,
}

impl ModelOperators {
    fn of(incidence: SparseMatrix, weights: &[f64]) -> Self {
        let laplacian = calc_weighted_laplacian(&incidence, weights);
        let branch_flow_matrix = calc_solver_branch_flow_matrix(&incidence, weights);
        let branch_incidence = incidence.transpose_view().to_csr();
        ModelOperators {
            incidence,
            branch_incidence,
            laplacian,
            branch_flow_matrix,
        }
    }
}

/// The stored `(column, value)` entries of row `row` of a CSR matrix, in
/// ascending column order.
pub(crate) fn csr_row(
    matrix: &SparseMatrix,
    row: usize,
) -> impl Iterator<Item = (usize, f64)> + '_ {
    let (columns, values) = matrix
        .outer_view(row)
        .map_or((&[][..], &[][..]), |view| view.into_raw_storage());
    columns.iter().copied().zip(values.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3, CASE3};
    use powerio_tx::POWER_MODELS_ANGLE_BOUND_PAD;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
    }

    /// A branch the source left unrated gets its bound from `Branch::synthesize_rate_a`,
    /// which reads both ends of each terminal's voltage band. Under a narrow angle window
    /// the widest phasor difference is one terminal at its ceiling and the other at its
    /// floor, so reading only the two ceilings (what tellegen did before powerio 0.9)
    /// returns a bound several times tighter than the branch physically has.
    #[test]
    fn an_unrated_branch_is_bounded_at_the_mixed_voltage_corner() {
        // CASE3 with one unrated branch (rate_a = 0) held to a 3 degree window.
        let case = CASE3.replace(
            " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
            " 1 2 0.01 0.1 0 0 0 0 0 0 1 -3 3;",
        );
        let net = crate::model::parse_matpower(&case).expect("parse");
        let dc = DcNetwork::from_network(&net).expect("build");

        // |Z| = hypot(0.01, 0.1); widest separation over the (0.9, 1.1) band box at a
        // 3 degree window; times the larger ceiling.
        let window = 3.0_f64.to_radians();
        let zmag = 0.01_f64.hypot(0.1);
        let sep = |vf: f64, vt: f64| (vf * vf + vt * vt - 2.0 * vf * vt * window.cos()).sqrt();
        approx(dc.fmax[0], 1.1 * sep(1.1, 0.9) / zmag);

        // The ceilings-only bound the old formula produced is more than three times
        // tighter, and an OPF enforces it.
        let ceilings_only = 1.1 * sep(1.1, 1.1) / zmag;
        assert!(
            dc.fmax[0] > 3.0 * ceilings_only,
            "expected the band bound {} to dwarf the ceilings-only bound {ceilings_only}",
            dc.fmax[0]
        );

        // The rated branches keep their stated limit.
        approx(dc.fmax[1], 2.5);
        approx(dc.fmax[2], 2.5);
    }

    /// An unconstrained window takes PowerIO's PowerModels pad (1.0472 rad,
    /// PowerModels' own constant), and an unrated branch's limit is
    /// synthesized over that padded window: at equal ceilings the widest
    /// separation is `2 vmax sin(pad / 2)`.
    #[test]
    fn unconstrained_unrated_branches_use_the_sixty_degree_window() {
        for (amin, amax) in [(-360, 360), (0, 0)] {
            let case = CASE3.replace(
                " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
                &format!(" 1 2 0.01 0.1 0 0 0 0 0 0 1 {amin} {amax};"),
            );
            let net = crate::model::parse_matpower(&case).expect("parse");
            let dc = DcNetwork::from_network(&net).expect("build");

            assert_eq!(dc.angmin[0], -POWER_MODELS_ANGLE_BOUND_PAD);
            assert_eq!(dc.angmax[0], POWER_MODELS_ANGLE_BOUND_PAD);
            let separation = 2.0 * 1.1 * (POWER_MODELS_ANGLE_BOUND_PAD / 2.0).sin();
            approx(dc.fmax[0], 1.1 * separation / 0.01_f64.hypot(0.1));
            approx(dc.fmax[1], 2.5);
            approx(dc.fmax[2], 2.5);
        }
    }

    #[test]
    fn lowered_three_winding_branches_use_the_sixty_degree_window() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse");
        let mut windings = [1, 2, 3].map(|bus| powerio::Winding::new(powerio::BusId(bus)));
        windings[0].rate_a = net.base_mva();
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva());
        net.transformers_3w_mut()
            .push(powerio::Transformer3W::new(windings, [impedance; 3]));

        let dc = DcNetwork::from_network(&net).expect("build");
        assert_eq!(dc.m, 6);
        approx(dc.fmax[3], 1.0);
        for branch in 3..6 {
            assert_eq!(dc.branch_source_rows[branch], None);
            // PowerIO owns the lowered winding rows and pads them with its
            // PowerModels angle window.
            approx(dc.angmin[branch], -POWER_MODELS_ANGLE_BOUND_PAD);
            approx(dc.angmax[branch], POWER_MODELS_ANGLE_BOUND_PAD);
            if branch > 3 {
                // PowerIO owns star lowering and synthesized limits. Tellegen
                // checks that every lowered unrated winding receives a finite
                // positive bound and that raw/normalized construction agrees below.
                assert!(dc.fmax[branch].is_finite() && dc.fmax[branch] > 0.0);
            }
        }

        let normalized = net.to_normalized().expect("normalize 3W case");
        let from_normalized = DcNetwork::from_network(&normalized).expect("build normalized 3W");
        assert_eq!(from_normalized.bus_ids, dc.bus_ids);
        assert_eq!(from_normalized.branch_ids, dc.branch_ids);
        assert_eq!(from_normalized.branch_source_rows, dc.branch_source_rows);
        for (actual, expected) in from_normalized.fmax.iter().zip(&dc.fmax) {
            approx(*actual, *expected);
        }
    }

    #[test]
    fn raw_and_normalized_inputs_build_the_same_dc_model() {
        let text = CASE3
            .replace(
                " 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;",
                " 2 1 90 30 10 0 1 1 0 230 1 1.1 0.9;",
            )
            .replace(
                " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
                " 1 2 0.01 0.1 0 175 175 175 1 15 1 -30 30;",
            );
        let raw = crate::model::parse_matpower(&text).expect("parse affine case");
        let normalized = raw.to_normalized().expect("normalize affine case");
        let a = DcNetwork::from_network(&raw).expect("build raw");
        let b = DcNetwork::from_network(&normalized).expect("build normalized");

        assert_eq!(a.bus_ids, b.bus_ids);
        assert_eq!(a.branch_ids, b.branch_ids);
        assert_eq!(a.gen_ids, b.gen_ids);
        assert_eq!(a.branch_source_rows, b.branch_source_rows);
        assert_eq!(a.gen_source_rows, b.gen_source_rows);
        for (left, right) in [
            (&a.prep.p_d, &b.prep.p_d),
            (&a.prep.g_s, &b.prep.g_s),
            (&a.prep.p_shift, &b.prep.p_shift),
            (&a.b, &b.b),
            #[cfg(feature = "sensitivity")]
            (&a.shift, &b.shift),
            (&a.fmax, &b.fmax),
            (&a.angmin, &b.angmin),
            (&a.angmax, &b.angmax),
            (&a.gmin, &b.gmin),
            (&a.gmax, &b.gmax),
            (&a.cq, &b.cq),
            (&a.cl, &b.cl),
            (&a.cc, &b.cc),
        ] {
            assert_eq!(left.len(), right.len());
            for (&actual, &expected) in left.iter().zip(right.iter()) {
                approx(actual, expected);
            }
        }
    }

    #[test]
    fn dimensions_and_ids() {
        let dc = parse_case3();
        assert_eq!(dc.n, 3);
        assert_eq!(dc.m, 3);
        assert_eq!(dc.k, 2);
        assert_eq!(dc.bus_ids, vec![1, 2, 3]);
        assert_eq!(dc.branch_ids, vec![1, 2, 3]);
        #[cfg(feature = "sensitivity")]
        assert_eq!(dc.branch_identities, vec!["1-2", "1-3", "2-3"]);
        assert_eq!(dc.gen_ids, vec![1, 2]);
        assert_eq!(dc.branch_source_rows, vec![Some(0), Some(1), Some(2)]);
        assert_eq!(dc.gen_source_rows, vec![Some(0), Some(1)]);
        approx(dc.base_mva, 100.0);
        // Bus 1 is the MATPOWER slack (type 3) -> dense index 0.
        assert_eq!(dc.ref_buses, [0]);
    }

    #[test]
    fn typed_instance_objective_and_constraint_masks_reach_the_workspace() {
        use powerio_prob::{ActiveConstraints, ConstraintSelection, Objective};

        let mut network = crate::model::parse_matpower(CASE3).expect("parse case3");
        network.buses_mut()[1].uid = Some("load-bus".into());
        network.generators_mut()[0].uid = Some("generator-a".into());
        network.branches_mut()[0].uid = Some("line-a".into());
        let mut constraints = ActiveConstraints::default();
        constraints.generator_capability = ConstraintSelection::None;
        constraints.thermal_limits = ConstraintSelection::Only(vec!["line-a".into()]);
        constraints.angle_bounds = ConstraintSelection::None;
        let instance = DcOpfInstance::from_network(network)
            .expect("instance")
            .with_objective(Objective::none())
            .with_constraints(constraints);

        let model = DcNetwork::from_instance(&instance).expect("workspace");
        assert_eq!(model.objective, PreparedObjective::Feasibility);
        assert_eq!(model.generator_capability_active, vec![false, false]);
        assert_eq!(model.thermal_limit_active, vec![true, false, false]);
        assert_eq!(model.angle_bound_active, vec![false, false, false]);
        assert_eq!(model.bus_uids[1].as_deref(), Some("load-bus"));
        #[cfg(feature = "sensitivity")]
        assert_eq!(model.branch_identities[0], "line-a");
        assert!(model.cq.iter().all(|value| *value == 0.0));
        assert!(model.cl.iter().all(|value| *value == 0.0));
        assert!(model.cc.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn ids_remain_source_order_after_filtering() {
        let mut net = crate::model::parse_matpower(crate::model::CASE3).expect("parse case3");
        net.branches_mut()[0].in_service = false;
        net.generators_mut()[0].in_service = false;

        let dc = DcNetwork::from_network(&net).expect("build filtered DcNetwork");

        assert_eq!(dc.branch_ids, vec![2, 3]);
        assert_eq!(dc.gen_ids, vec![2]);
        assert_eq!(dc.branch_source_rows, vec![Some(1), Some(2)]);
        assert_eq!(dc.gen_source_rows, vec![Some(1)]);
    }

    #[test]
    fn phase_shifter_and_shunt_use_the_complete_affine_instance() {
        let text = CASE3
            .replace(
                " 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;",
                " 2 1 90 30 10 0 1 1 0 230 1 1.1 0.9;",
            )
            .replace(
                " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
                " 1 2 0.01 0.1 0 0 0 0 1 15 1 -30 30;",
            );
        let mut net = crate::model::parse_matpower(&text).expect("parse affine case");
        let dc = DcNetwork::from_network(&net).expect("build affine case");

        approx(dc.prep.p_d[1], 0.9);
        approx(dc.prep.g_s[1], 0.1);
        approx(dc.flow_offset[0], dc.b[0] * 15.0_f64.to_radians());
        // The 1-2 shifter withdraws its offset at bus 1 and injects it at bus 2.
        let phase = &dc.prep.p_shift;
        approx(phase[0], dc.flow_offset[0]);
        approx(phase[1], -dc.flow_offset[0]);
        approx(phase[2], 0.0);
        let fixed = dc.current_fixed_withdrawal();
        for i in 0..dc.n {
            approx(fixed[i], dc.prep.p_d[i] + dc.prep.g_s[i] + phase[i]);
        }
        assert!(dc.fmax[0].is_finite() && dc.fmax[0] > 0.0);

        // Taking the phase-shifting branch out of service removes both affine
        // terms. Demand and shunt withdrawal remain at bus 2.
        net.branches_mut()[0].in_service = false;
        let dc = DcNetwork::from_network(&net).expect("build without the shifter");
        assert!(dc.flow_offset.iter().all(|offset| *offset == 0.0));
        let fixed = dc.current_fixed_withdrawal();
        approx(fixed[0], 0.0);
        approx(fixed[1], 1.0);
        approx(fixed[2], 0.0);
    }

    #[test]
    fn susceptance_is_a_grounded_laplacian() {
        let dc = parse_case3();
        // b = -x / (r^2 + x^2) for every identical line.
        let w = 0.1 / (0.01 * 0.01 + 0.1 * 0.1); // = -b = 9.9009901...
        for &be in &dc.b {
            approx(be, -w);
        }
        // Reassemble B and check the Laplacian structure: symmetric, every row
        // sums to zero, off-diagonals are -w for each of the three lines, and
        // each bus (degree 2) has diagonal 2w.
        let mut dense = [[0.0f64; 3]; 3];
        for (r, row) in dense.iter_mut().enumerate() {
            for (c, v) in csr_row(&dc.laplacian, r) {
                row[c] = v;
            }
        }
        for (i, row) in dense.iter().enumerate() {
            approx(row[i], 2.0 * w);
            let row_sum: f64 = row.iter().sum();
            approx(row_sum, 0.0);
            for (j, &value) in row.iter().enumerate() {
                approx(value, dense[j][i]);
                if i != j {
                    approx(value, -w);
                }
            }
        }
    }

    #[test]
    fn per_unit_demand_and_limits() {
        let dc = parse_case3();
        // 90 MW load at bus 2 (dense index 1), per unit on a 100 MVA base.
        approx(dc.prep.p_d[0], 0.0);
        approx(dc.prep.p_d[1], 0.9);
        approx(dc.prep.p_d[2], 0.0);
        // rate_a 250 MW -> 2.5 per unit on every line.
        for &fm in &dc.fmax {
            approx(fm, 2.5);
        }
        // pmax/pmin per unit.
        approx(dc.gmax[0], 2.5);
        approx(dc.gmax[1], 2.7);
        approx(dc.gmin[0], 0.1);
        approx(dc.gmin[1], 0.1);
    }

    #[test]
    fn quadratic_costs_in_per_unit() {
        let dc = parse_case3();
        // c2 scales by base^2, c1 by base (the per-unit cost rescale).
        approx(dc.cq[0], 0.11 * 100.0 * 100.0); // 1100
        approx(dc.cl[0], 5.0 * 100.0); // 500
        approx(dc.cq[1], 0.085 * 100.0 * 100.0); // 850
        approx(dc.cl[1], 1.2 * 100.0); // 120
                                       // Shedding cost = 10 x max marginal cost (2 cq gmax + cl).
        let marginal = (2.0 * 1100.0 * 2.5 + 500.0_f64).max(2.0 * 850.0 * 2.7 + 120.0);
        for &cs in &dc.c_shed {
            approx(cs, 10.0 * marginal);
        }
    }

    #[test]
    fn piecewise_costs_remain_exact_segment_lines() {
        let text = CASE3
            .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 1 100 3 200 7;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 100 50;");
        let net = crate::model::parse_matpower(&text).expect("parse piecewise case3");
        let dc = DcNetwork::from_network(&net).expect("build piecewise DcNetwork");
        approx(dc.cq[0], 0.0);
        approx(dc.cl[0], 0.0);
        approx(dc.cq[1], 0.0);
        approx(dc.cl[1], 0.0);

        let first = dc.piecewise_costs[0].as_ref().expect("first cost");
        assert_eq!(first.slopes, vec![2.0, 4.0]);
        assert_eq!(first.intercepts, vec![1.0, -1.0]);
        approx(dc.generator_cost(0, 0.5), 2.0);
        approx(dc.generator_cost(0, 1.5), 5.0);

        let second = dc.piecewise_costs[1].as_ref().expect("second cost");
        assert_eq!(second.slopes, vec![50.0]);
        assert_eq!(second.intercepts, vec![0.0]);
        approx(dc.generator_cost(1, 0.75), 37.5);
    }

    #[test]
    fn texas_literal_nonconvex_piecewise_rows_are_rejected() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/ACTIVSg7000/Texas7k_20210804.m"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping Texas piecewise validation: {path} not found");
            return;
        };
        let network = crate::model::parse_matpower(&text).expect("parse Texas7k");
        let error = match DcNetwork::from_network(&network) {
            Ok(_) => panic!("Texas7k contains literal decreasing segment slopes"),
            Err(error) => error,
        };
        assert!(
            error.contains("BUILD.INSTANCE.PIECEWISE_COST_NONCONVEX")
                && error.contains("nonconvex piecewise linear cost"),
            "unexpected Texas7k preparation failure: {error}"
        );
    }

    #[test]
    fn angle_bounds_default_to_sixty_degrees() {
        let dc = parse_case3();
        // The +-360 degree MATPOWER default collapses to PowerIO's +-60 degree
        // PowerModels window.
        let pad = POWER_MODELS_ANGLE_BOUND_PAD;
        for e in 0..dc.m {
            approx(dc.angmin[e], -pad);
            approx(dc.angmax[e], pad);
        }
    }

    #[test]
    fn builds_on_a_real_case() {
        // Real-case smoke check: ACTIVSg200 exercises to_normalized, reference
        // inference, synthesized unrated limits, and cost shaping on a full network —
        // the parity target for step 5. Skips when the data directory is absent.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/ACTIVSg200/case_ACTIVSg200.m"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping builds_on_a_real_case: {path} not found");
            return;
        };
        let net = crate::model::parse_matpower(&text).expect("parse ACTIVSg200");
        let dc = DcNetwork::from_network(&net).expect("build DcNetwork from ACTIVSg200");

        assert!(dc.n > 0 && dc.m > 0 && dc.k > 0);
        assert!(dc.ref_buses.iter().all(|&r| r < dc.n));
        assert_eq!(dc.bus_ids.len(), dc.n);
        assert_eq!(dc.prep.p_d.len(), dc.n);
        assert_eq!(dc.c_shed.len(), dc.n);
        assert_eq!(dc.fmax.len(), dc.m);
        assert_eq!(dc.angmin.len(), dc.m);
        assert_eq!(dc.gmax.len(), dc.k);
        assert_eq!(dc.gen_bus.len(), dc.k);
        for &fm in &dc.fmax {
            assert!(fm > 0.0 && fm.is_finite(), "thermal limit {fm}");
        }
        for &be in &dc.b {
            assert!(be.is_finite(), "susceptance {be}");
        }
        for &d in &dc.prep.p_d {
            assert!(d.is_finite());
        }
        // B is a grounded Laplacian regardless of connectivity: rows sum to zero.
        let row_sum: Vec<f64> = (0..dc.n)
            .map(|r| csr_row(&dc.laplacian, r).map(|(_, v)| v).sum())
            .collect();
        for (i, s) in row_sum.iter().enumerate() {
            assert!(s.abs() < 1e-5, "B row {i} sums to {s}");
        }
    }

    #[test]
    fn near_zero_impedance_jumper_is_a_tie_not_an_open_circuit() {
        // Regression for a CATS-specific DC OPF bug: a branch with tiny but nonzero
        // impedance (a substation bus-splitting jumper, common in detailed synthetic
        // cases — CaliforniaTestSystem.m has 11 of them, r and x both ~1e-6 to 1e-7 pu)
        // was falling below the old `MIN_Z_SQUARED = 1e-10` guard and getting `b = 0`,
        // i.e. treated as an open circuit. That silently disconnected the two buses in
        // the B-theta model, wrongly restricting which paths generation could reach and
        // costing CATS's DC OPF about $1,131/h (0.15%) versus the PowerModels.jl/IPOPT
        // reference. The branch is a real, near-ideal tie, not a break in the network:
        // it must carry a correspondingly large susceptance, not zero.
        let text = CASE3.replace(
            "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
            "1 3 1e-7 1e-6 0 250 250 250 0 0 1 -360 360;",
        );
        let net = crate::model::parse_matpower(&text).expect("parse jumper case3");
        let dc = DcNetwork::from_network(&net).expect("build DcNetwork with jumper branch");

        let z2 = 1e-7_f64.powi(2) + 1e-6_f64.powi(2);
        let expected_b = -1e-6 / z2;
        approx(dc.b[1], expected_b); // branch index 1 is the 1-3 jumper
        assert!(
            dc.b[1].abs() > 1e5,
            "jumper susceptance {} should be large, not the near-zero open-circuit value",
            dc.b[1]
        );
    }
}
