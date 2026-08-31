//! AC network data in the vectorized pi-model admittance form. Carries per-branch
//! series and shunt admittance with transformer tap and phase shift, the per-bus
//! shunt, the real and reactive demand, and the generator injection aggregated to
//! buses — everything the polar AC power flow and its voltage sensitivities read,
//! plus the per-generator bounds and costs the conic OPF optimizes. Built from a
//! powerio `BalancedNetwork` exactly as [`DcNetwork`](super::DcNetwork) is. Gated with the
//! faer paths behind `sensitivity`.

use std::collections::BTreeMap;

use num_complex::Complex;
use powerio::{BalancedNetwork, LoadVoltageModel};
#[cfg(feature = "conic")]
use powerio_matrix::PreparedObjective;
use powerio_matrix::{build_ac_opf_preparation, AcOpfAssemblyOptions, Units};
use powerio_prob::{AcBusSpecification, AcOpfInstance, AcPfInstance};

#[cfg(feature = "conic")]
use super::{
    normalize_angle_bounds, reject_unsupported_active_elements, uids_for_source_rows,
    validate_canonical_identity, PiecewiseCost,
};
use super::{normalize_for_model, reconstruct_ids, Ids};

const NEAR_ZERO_IMPEDANCE_SQUARED: f64 = 1.0e-10;

#[cfg(feature = "conic")]
fn stable_source_ids(
    source_rows: &[Option<usize>],
    source_len: usize,
    family: &str,
) -> Result<Vec<usize>, String> {
    let mut next_synthetic = source_len
        .checked_add(1)
        .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
    source_rows
        .iter()
        .map(|row| match row {
            Some(row) => row
                .checked_add(1)
                .ok_or_else(|| format!("{family} id space exhausted")),
            None => {
                let id = next_synthetic;
                next_synthetic = next_synthetic
                    .checked_add(1)
                    .ok_or_else(|| format!("{family} synthetic id space exhausted"))?;
                Ok(id)
            }
        })
        .collect()
}

/// AC network data in the vectorized pi-model admittance form.
///
/// Each branch contributes the standard pi-model stamp built from its series
/// admittance `y = g + j b`, the complex tap `t = tap · e^{j·shift}`, and the
/// from/to line-charging shunts; [`AcNetwork::ybus`] assembles the bus admittance
/// matrix `Y`. The net injection at bus `i` is `S_i = V_i · conj((Y V)_i)`, equal
/// to `(pg_i − pd_i) + j(qg_i − qd_i)`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub(crate) struct AcNetwork {
    /// Buses and branches after filtering (in-service, non-isolated).
    pub n: usize,
    pub m: usize,
    /// Branch endpoints in dense bus-index space.
    pub br_from: Vec<usize>,
    pub br_to: Vec<usize>,
    /// Series conductance `g = r/(r²+x²)` and susceptance `b = −x/(r²+x²)` per
    /// retained branch. A literal zero-impedance row has undefined admittance
    /// and is rejected at model construction; a tiny but nonzero impedance keeps
    /// its true, correspondingly large admittance.
    pub g: Vec<f64>,
    pub b: Vec<f64>,
    /// From/to-side shunt admittance (line charging). Carries PowerIO's canonical
    /// per-terminal values; a legacy MATPOWER `br_b` is derived as the symmetric
    /// special case.
    pub g_fr: Vec<f64>,
    pub b_fr: Vec<f64>,
    pub g_to: Vec<f64>,
    pub b_to: Vec<f64>,
    /// Transformer tap magnitude (`1` for a plain line) and phase shift (radians).
    pub tap: Vec<f64>,
    pub shift: Vec<f64>,
    /// Per-unit apparent-power thermal limit per branch (`rate_a`; a large sentinel
    /// stands in for an unlimited `rate_a == 0` branch). The conic OPF caps each
    /// branch's `|S|` at this with a second-order cone.
    #[cfg(feature = "conic")]
    pub rate_a: Vec<f64>,
    /// Per-branch voltage-angle-difference bounds (radians): `va_from − va_to ∈
    /// [angmin, angmax]`. Normalized to the ±60° MATPOWER/PowerModels convention when
    /// the source leaves them unset or unconstrained (shared with the DC model). The AC
    /// OPF enforces these as linear inequalities; the conic SOCWR maps them onto the
    /// W-space products `wr`/`wi`.
    #[cfg(feature = "conic")]
    pub angmin: Vec<f64>,
    #[cfg(feature = "conic")]
    pub angmax: Vec<f64>,
    /// Branch switching state (1 closed, 0 open). All branches start closed.
    pub sw: Vec<f64>,
    /// Per-bus shunt admittance (per unit): conductance `gs`, susceptance `bs`.
    pub gs: Vec<f64>,
    pub bs: Vec<f64>,
    /// Per-bus real and reactive demand (per unit).
    pub pd: Vec<f64>,
    pub qd: Vec<f64>,
    /// Per-bus aggregated scheduled generation (per unit), the power flow operating
    /// point.
    pub pg: Vec<f64>,
    pub qg: Vec<f64>,
    /// Generator count.
    pub k: usize,
    /// Bus each generator injects at, dense index.
    pub gen_bus: Vec<usize>,
    /// Per-unit generator real and reactive output bounds, per generator. The
    /// conic OPF optimizes over these; the power flow uses the per-bus aggregates.
    #[cfg(feature = "conic")]
    pub pmin: Vec<f64>,
    #[cfg(feature = "conic")]
    pub pmax: Vec<f64>,
    pub qmin: Vec<f64>,
    pub qmax: Vec<f64>,
    /// Per-unit generation cost `cq[g] pg² + cl[g] pg + cc[g]` per generator.
    #[cfg(feature = "conic")]
    pub cq: Vec<f64>,
    #[cfg(feature = "conic")]
    pub cl: Vec<f64>,
    #[cfg(feature = "conic")]
    pub cc: Vec<f64>,
    /// Exact convex piecewise linear costs aligned with generator columns.
    #[cfg(feature = "conic")]
    pub(crate) piecewise_costs: Vec<Option<PiecewiseCost>>,
    /// Dense generator index -> original source generator id.
    pub gen_ids: Vec<usize>,
    /// Per-bus voltage magnitude bounds, and the per-bus magnitude setpoint: the
    /// regulating generator's voltage setpoint (`vg`) at PV and slack buses, the bus
    /// voltage elsewhere. The power flow holds PV/slack magnitudes at this value; it also
    /// seeds the flat start.
    #[cfg(feature = "conic")]
    pub vm_min: Vec<f64>,
    #[cfg(feature = "conic")]
    pub vm_max: Vec<f64>,
    pub vm_set: Vec<f64>,
    /// Reference (slack) bus, dense index.
    pub slack: usize,
    /// Buses whose AC power flow magnitude is prescribed. The reference is
    /// represented separately by `slack`.
    pub(crate) pf_pv: Vec<bool>,
    /// Dense index -> original source id, as in [`DcNetwork`](super::DcNetwork).
    pub bus_ids: Vec<usize>,
    pub branch_ids: Vec<usize>,
    pub(crate) bus_source_rows: Vec<Option<usize>>,
    /// Dense index -> powerio row uid (`None` when the source network carried no
    /// uids), as in [`DcNetwork`](super::DcNetwork).
    pub bus_uids: Vec<Option<String>>,
    #[cfg(feature = "conic")]
    pub branch_uids: Vec<Option<String>>,
    /// System base power (MVA).
    pub base_mva: f64,
    /// Objective and active constraints compiled from the PowerIO instance.
    #[cfg(feature = "conic")]
    pub(crate) objective: PreparedObjective,
    #[cfg(feature = "conic")]
    pub(crate) voltage_bound_active: Vec<bool>,
    #[cfg(feature = "conic")]
    pub(crate) generator_capability_active: Vec<bool>,
    #[cfg(feature = "conic")]
    pub(crate) thermal_limit_active: Vec<bool>,
    #[cfg(feature = "conic")]
    pub(crate) angle_bound_active: Vec<bool>,
    /// An active generator names a different regulated bus. The SOCWR model
    /// ignores voltage-control actions; ACPF rejects this explicitly.
    pub has_remote_voltage_control: bool,
}

impl AcNetwork {
    /// Build the private AC power flow workspace from the supplied PowerIO
    /// instance. Boundary specifications, not inferred generator placement,
    /// determine the PQ, PV, and reference equations.
    pub fn from_pf_instance(instance: &AcPfInstance) -> Result<AcNetwork, String> {
        let mut model = Self::from_network(instance.network())?;
        if instance.specifications().len() != instance.network().buses().len() {
            return Err(format!(
                "AC power flow instance has {} bus specifications for {} buses",
                instance.specifications().len(),
                instance.network().buses().len()
            ));
        }

        model.pf_pv.fill(false);
        let mut reference = None;
        for (dense, &source_id) in model.bus_ids.iter().enumerate() {
            let source_row = model.bus_source_rows[dense].ok_or_else(|| {
                format!(
                    "synthetic AC preparation bus {source_id} has no source power flow specification"
                )
            })?;
            let specification = instance.specifications()[source_row];
            match specification {
                AcBusSpecification::Pq { p, q } => {
                    model.pg[dense] = model.pd[dense] + p / model.base_mva;
                    model.qg[dense] = model.qd[dense] + q / model.base_mva;
                }
                AcBusSpecification::Pv { p, vm } => {
                    model.pg[dense] = model.pd[dense] + p / model.base_mva;
                    model.vm_set[dense] = vm;
                    model.pf_pv[dense] = true;
                }
                AcBusSpecification::Reference { vm, va } => {
                    if va.abs() > 1e-12 {
                        return Err(format!(
                            "AC power flow reference bus {source_id} states angle {va} degrees; Tellegen currently requires zero"
                        ));
                    }
                    if reference.replace(dense).is_some() {
                        return Err(
                            "AC power flow instance states more than one reference bus".to_owned()
                        );
                    }
                    model.vm_set[dense] = vm;
                }
                AcBusSpecification::Isolated => {
                    return Err(format!(
                        "isolated AC power flow bus {source_id} entered the analysis preparation"
                    ));
                }
                _ => {
                    return Err(format!(
                        "AC power flow bus {source_id} uses an unsupported specification"
                    ));
                }
            }
        }
        model.slack = reference.ok_or("AC power flow instance has no reference bus")?;
        model.pf_pv[model.slack] = false;
        Ok(model)
    }

    /// Build the private AC solver workspace from the supplied PowerIO
    /// instance, preserving its declared objective and constraint selections.
    #[cfg(feature = "conic")]
    pub fn from_instance(instance: &AcOpfInstance) -> Result<AcNetwork, String> {
        let raw = instance.network();
        validate_canonical_identity(raw)?;
        reject_unsupported_active_elements(raw)?;
        let voltage_dependent_loads = raw
            .loads()
            .iter()
            .filter(|load| {
                load.in_service
                    && !matches!(
                        load.voltage_model.as_ref(),
                        None | Some(LoadVoltageModel::ConstantPower)
                    )
            })
            .count();
        if voltage_dependent_loads > 0 {
            return Err(format!(
                "network contains {voltage_dependent_loads} active voltage-dependent load(s); SOCWR supports constant-power loads only"
            ));
        }
        let mut assembly = AcOpfAssemblyOptions::default();
        assembly.units = Units::PerUnit;
        assembly.skip_zero_impedance = false;
        assembly.synthesize_unrated_limits = true;
        let prep = build_ac_opf_preparation(instance, &assembly)
            .map_err(|e| format!("{}: {e}", e.code().code))?;
        let n = prep.n_buses;
        let m = prep.n_branches();
        let k = prep.n_generators();
        if k == 0 {
            return Err("network has no in-service generators".to_owned());
        }
        for branch in 0..m {
            if !prep.branches.angle_bound_active[branch] {
                continue;
            }
            let min = prep.branches.angle_min[branch];
            let max = prep.branches.angle_max[branch];
            let covers_principal_circle =
                min <= -std::f64::consts::PI && max >= std::f64::consts::PI;
            if !covers_principal_circle
                && (min <= -std::f64::consts::FRAC_PI_2 || max >= std::f64::consts::FRAC_PI_2)
            {
                return Err(format!(
                    "SOCWR cannot represent active angle bounds [{min}, {max}] on branch {} outside (-pi/2, pi/2)",
                    prep.branches.identities[branch]
                ));
            }
        }

        let bus_ids: Vec<usize> = prep.bus_ids.iter().map(|id| id.0).collect();
        let bus_uids =
            uids_for_source_rows(&prep.bus_source_rows, raw.buses(), |bus| &bus.uid, "bus")?;
        let branch_uids = uids_for_source_rows(
            &prep.branches.source_rows,
            raw.branches(),
            |branch| &branch.uid,
            "branch",
        )?;
        let branch_ids =
            stable_source_ids(&prep.branches.source_rows, raw.branches().len(), "branch")?;
        let gen_ids = stable_source_ids(
            &prep.generators.source_rows,
            raw.generators().len(),
            "generator",
        )?;
        let slack = prep.reference_buses.single().map_err(|e| e.to_string())?;
        let mut vm_set = prep.calc_vm_setpoints();
        let mut pg = vec![0.0; n];
        let mut qg = vec![0.0; n];
        let mut pf_pv = vec![false; n];
        for (generator, &bus) in prep.generators.bus_of_gen.iter().enumerate() {
            pf_pv[bus] = true;
            pg[bus] += prep.generators.pg[generator];
            qg[bus] += prep.generators.qg[generator];
            let vg = prep.generators.vg[generator];
            if vg > 0.0 {
                let lo = prep.buses.vm_min[bus];
                let hi = prep.buses.vm_max[bus];
                vm_set[bus] = if lo.is_finite() && hi.is_finite() && lo <= hi {
                    vg.clamp(lo, hi)
                } else {
                    vg
                };
            }
        }
        pf_pv[slack] = false;
        let has_remote_voltage_control = raw.generators().iter().any(|generator| {
            generator.in_service
                && generator
                    .regulated_bus
                    .is_some_and(|regulated| regulated != generator.bus)
        });

        Ok(AcNetwork {
            n,
            m,
            br_from: prep.branches.from_bus.clone(),
            br_to: prep.branches.to_bus.clone(),
            g: prep.branches.g.clone(),
            b: prep.branches.b.clone(),
            g_fr: prep.branches.g_fr.clone(),
            b_fr: prep.branches.b_fr.clone(),
            g_to: prep.branches.g_to.clone(),
            b_to: prep.branches.b_to.clone(),
            tap: prep.branches.tap.clone(),
            shift: prep.branches.shift.clone(),
            rate_a: prep.branches.s_max.clone(),
            angmin: prep.branches.angle_min.clone(),
            angmax: prep.branches.angle_max.clone(),
            sw: vec![1.0; m],
            gs: prep.buses.g_s.clone(),
            bs: prep.buses.b_s.clone(),
            pd: prep.buses.p_d.clone(),
            qd: prep.buses.q_d.clone(),
            pg,
            qg,
            k,
            gen_bus: prep.generators.bus_of_gen.clone(),
            pmin: prep.generators.pmin.clone(),
            pmax: prep.generators.pmax.clone(),
            qmin: prep.generators.qmin.clone(),
            qmax: prep.generators.qmax.clone(),
            cq: prep.generators.q.iter().map(|value| value / 2.0).collect(),
            cl: prep.generators.c.clone(),
            cc: prep.generators.c0.clone(),
            piecewise_costs: prep
                .generators
                .piecewise_linear
                .clone()
                .into_iter()
                .map(|cost| cost.map(PiecewiseCost::from_prepared))
                .collect(),
            gen_ids,
            vm_min: prep.buses.vm_min.clone(),
            vm_max: prep.buses.vm_max.clone(),
            vm_set,
            slack,
            pf_pv,
            bus_ids,
            branch_ids,
            bus_source_rows: prep.bus_source_rows.clone(),
            bus_uids,
            branch_uids,
            base_mva: prep.base_mva,
            objective: prep.objective,
            voltage_bound_active: prep.buses.voltage_bound_active.clone(),
            generator_capability_active: prep.generators.capability_active.clone(),
            thermal_limit_active: prep.branches.thermal_limit_active.clone(),
            angle_bound_active: prep.branches.angle_bound_active.clone(),
            has_remote_voltage_control,
        })
    }

    /// Whether an active angle selection produces a nonredundant convex row
    /// in W space. Bounds spanning the whole principal angle circle are exact
    /// no-ops; narrower bounds outside ±pi/2 are rejected at construction.
    #[cfg(feature = "conic")]
    pub(crate) fn conic_angle_bound_active(&self, branch: usize) -> bool {
        self.angle_bound_active[branch]
            && !(self.angmin[branch] <= -std::f64::consts::PI
                && self.angmax[branch] >= std::f64::consts::PI)
    }

    /// Evaluate one declared generator cost at `power` in per unit.
    #[cfg(feature = "conic")]
    pub(crate) fn generator_cost(&self, generator: usize, power: f64) -> f64 {
        self.piecewise_costs[generator].as_ref().map_or_else(
            || self.cq[generator] * power * power + self.cl[generator] * power + self.cc[generator],
            |cost| cost.evaluate(power),
        )
    }

    /// Build the AC model from a parsed powerio `BalancedNetwork`, normalizing through
    /// `BalancedNetwork::to_normalized` (per unit, radians, filtered, densely reindexed,
    /// reference inferred) and reading its nodal and generator data from a
    /// `powerio-prob` [`AcOpfInstance`](powerio_prob::AcOpfInstance): per-unit demand,
    /// generator PQ bounds and scheduled output, voltage bands and setpoints. The
    /// instance owns the complete complex pi model and preserves its quadratic
    /// or convex piecewise linear objective, including per-terminal charging and
    /// 3-winding star lowering. Tellegen layers only its `rate_a == 0` cone
    /// sentinel and angle-bound policy on top.
    pub fn from_network(raw: &BalancedNetwork) -> Result<AcNetwork, String> {
        let voltage_dependent_loads = raw
            .loads()
            .iter()
            .filter(|load| {
                load.in_service
                    && !matches!(
                        load.voltage_model.as_ref(),
                        None | Some(LoadVoltageModel::ConstantPower)
                    )
            })
            .count();
        if voltage_dependent_loads > 0 {
            return Err(format!(
                "network contains {voltage_dependent_loads} active voltage-dependent load(s); ACPF/SOCWR support constant-power loads only"
            ));
        }
        let has_remote_voltage_control = raw.generators().iter().any(|generator| {
            generator.in_service
                && generator
                    .regulated_bus
                    .is_some_and(|regulated| regulated != generator.bus)
        });
        let input = normalize_for_model(raw)?;
        let norm = input.network;
        let source_rows = input.source_rows;

        let instance = AcOpfInstance::from_network(norm).map_err(|e| e.to_string())?;
        let mut assembly = AcOpfAssemblyOptions::default();
        assembly.units = Units::PerUnit;
        // Omitting a zero impedance row would also omit its topology and
        // terminal charging. Fail closed instead of returning a plausible
        // solve for a different network.
        assembly.skip_zero_impedance = false;
        let prep = build_ac_opf_preparation(&instance, &assembly)
            .map_err(|e| format!("{}: {e}", e.code().code))?;

        let Ids {
            n,
            m,
            k,
            bus_ids,
            branch_ids,
            gen_ids,
            bus_uids,
            #[cfg(feature = "conic")]
            branch_uids,
        } = reconstruct_ids(
            raw,
            &prep.bus_ids,
            &prep.branches.analysis_rows,
            &prep.generators.analysis_rows,
            &source_rows,
        )?;

        let mut vm_set = prep.calc_vm_setpoints();
        let slack = prep.reference_buses.single().map_err(|e| e.to_string())?;

        // Move the complete PowerIO problem columns out of the one-shot
        // preparation. Its bus shunts already include folded self-loop pi
        // stamps, and its active branch columns carry canonical per-terminal
        // charging.
        let buses = prep.buses;
        let generators = prep.generators;
        let branches = prep.branches;
        let bus_source_rows = source_rows.buses.clone();
        #[cfg(feature = "conic")]
        let objective = prep.objective;
        #[cfg(feature = "conic")]
        let voltage_bound_active = buses.voltage_bound_active.clone();
        #[cfg(feature = "conic")]
        let generator_capability_active = generators.capability_active.clone();
        #[cfg(feature = "conic")]
        let thermal_limit_active = branches.thermal_limit_active.clone();
        #[cfg(feature = "conic")]
        let angle_bound_active = branches.angle_bound_active.clone();
        debug_assert_eq!(n, buses.p_d.len());
        debug_assert_eq!(m, branches.g.len());
        debug_assert_eq!(k, generators.q.len());

        let pd = buses.p_d;
        let qd = buses.q_d;
        let gs = buses.g_s;
        let bs = buses.b_s;
        let vm_min = buses.vm_min;
        let vm_max = buses.vm_max;

        let br_from = branches.from_bus;
        let br_to = branches.to_bus;
        let suppress_terminal_charging: Vec<bool> = branches
            .g
            .iter()
            .zip(&branches.b)
            .map(|(&g, &b)| {
                let admittance_squared = g * g + b * b;
                admittance_squared > 0.0
                    && admittance_squared.recip() <= NEAR_ZERO_IMPEDANCE_SQUARED
            })
            .collect();
        let g = branches.g;
        let b = branches.b;
        let suppress = |values: Vec<f64>| {
            values
                .into_iter()
                .zip(&suppress_terminal_charging)
                .map(|(value, &suppressed)| if suppressed { 0.0 } else { value })
                .collect()
        };
        // Detailed substation cases use near-ideal jumper rows. Retain their
        // physical series admittance but suppress terminal charging, matching
        // Tellegen's established CATS stability policy.
        let g_fr = suppress(branches.g_fr);
        let b_fr = suppress(branches.b_fr);
        let g_to = suppress(branches.g_to);
        let b_to = suppress(branches.b_to);
        let tap = branches.tap;
        let shift = branches.shift;
        #[cfg(feature = "conic")]
        let rate_a = branches
            .s_max
            .into_iter()
            .map(|rate| if rate > 0.0 { rate } else { 1.0e3 })
            .collect();
        #[cfg(feature = "conic")]
        let (angmin, angmax): (Vec<_>, Vec<_>) = branches
            .angle_min
            .into_iter()
            .zip(branches.angle_max)
            .map(|(min, max)| normalize_angle_bounds(min, max))
            .unzip();
        let sw = vec![1.0; m];

        // Per-bus scheduled generation for power flow, plus generator-space bounds
        // and costs for SOCWR. PowerIO states the quadratic as `0.5*q*p^2`.
        let mut pg = vec![0.0; n];
        let mut qg = vec![0.0; n];
        let mut pf_pv = vec![false; n];
        let gen_bus = generators.bus_of_gen;
        for (i, bus) in gen_bus.iter().copied().enumerate() {
            pf_pv[bus] = true;
            pg[bus] += generators.pg[i];
            qg[bus] += generators.qg[i];
            // Regulate this bus's magnitude to the generator's voltage setpoint, clamped
            // into the bus magnitude band: the power flow holds PV/slack magnitudes at
            // `vm_set` with no `vm` column to bound them, so an out-of-band `vg` would pin
            // the bus at an infeasible magnitude and the sensitivity would linearize there.
            let vg = generators.vg[i];
            if vg > 0.0 {
                // `f64::clamp` panics on an inverted or non-finite band, and nothing
                // between the case file and here establishes that the bus carries one:
                // the readers copy VMIN/VMAX verbatim and normalization leaves them
                // alone. An unusable band means no bound to apply, not a panic.
                let (lo, hi) = (vm_min[bus], vm_max[bus]);
                vm_set[bus] = if lo.is_finite() && hi.is_finite() && lo <= hi {
                    vg.clamp(lo, hi)
                } else {
                    vg
                };
            }
        }
        #[cfg(feature = "conic")]
        let cq = generators.q.into_iter().map(|value| value / 2.0).collect();
        #[cfg(feature = "conic")]
        let cl = generators.c;
        #[cfg(feature = "conic")]
        let cc = generators.c0;
        #[cfg(feature = "conic")]
        let piecewise_costs = generators
            .piecewise_linear
            .into_iter()
            .map(|cost| cost.map(PiecewiseCost::from_prepared))
            .collect();
        #[cfg(feature = "conic")]
        let pmin = generators.pmin;
        #[cfg(feature = "conic")]
        let pmax = generators.pmax;
        let qmin = generators.qmin;
        let qmax = generators.qmax;

        Ok(AcNetwork {
            n,
            m,
            br_from,
            br_to,
            g,
            b,
            g_fr,
            b_fr,
            g_to,
            b_to,
            tap,
            shift,
            #[cfg(feature = "conic")]
            rate_a,
            #[cfg(feature = "conic")]
            angmin,
            #[cfg(feature = "conic")]
            angmax,
            sw,
            gs,
            bs,
            pd,
            qd,
            pg,
            qg,
            k,
            gen_bus,
            #[cfg(feature = "conic")]
            pmin,
            #[cfg(feature = "conic")]
            pmax,
            qmin,
            qmax,
            #[cfg(feature = "conic")]
            cq,
            #[cfg(feature = "conic")]
            cl,
            #[cfg(feature = "conic")]
            cc,
            #[cfg(feature = "conic")]
            piecewise_costs,
            gen_ids,
            #[cfg(feature = "conic")]
            vm_min,
            #[cfg(feature = "conic")]
            vm_max,
            vm_set,
            slack,
            pf_pv,
            bus_ids,
            branch_ids,
            bus_source_rows,
            bus_uids,
            #[cfg(feature = "conic")]
            branch_uids,
            base_mva: raw.base_mva(),
            #[cfg(feature = "conic")]
            objective,
            #[cfg(feature = "conic")]
            voltage_bound_active,
            #[cfg(feature = "conic")]
            generator_capability_active,
            #[cfg(feature = "conic")]
            thermal_limit_active,
            #[cfg(feature = "conic")]
            angle_bound_active,
            has_remote_voltage_control,
        })
    }

    /// The complex bus admittance matrix `Y` as summed `(row, col, value)`
    /// triplets in `(row, col)` order. Each branch stamps its pi-model
    /// coefficients
    ///
    /// ```text
    /// yff = (y + y_fr) / tap²     yft = −y / conj(t)
    /// ytf = −y / t                ytt =  y + y_to
    /// ```
    ///
    /// scaled by the switching state, with `y = g + j b`, `t = tap · e^{j·shift}`,
    /// `y_fr = g_fr + j b_fr`, `y_to = g_to + j b_to`; the bus shunt `gs + j bs`
    /// lands on the diagonal. Open (`sw = 0`) branches contribute nothing.
    pub fn ybus(&self) -> Vec<(usize, usize, Complex<f64>)> {
        let mut acc: BTreeMap<(usize, usize), Complex<f64>> = BTreeMap::new();
        for i in 0..self.n {
            *acc.entry((i, i)).or_default() += Complex::new(self.gs[i], self.bs[i]);
        }
        for e in 0..self.m {
            if self.sw[e] == 0.0 {
                continue;
            }
            let (yff, yft, ytf, ytt) = self.branch_admittance(e);
            let (f, t) = (self.br_from[e], self.br_to[e]);
            *acc.entry((f, f)).or_default() += yff;
            *acc.entry((f, t)).or_default() += yft;
            *acc.entry((t, f)).or_default() += ytf;
            *acc.entry((t, t)).or_default() += ytt;
        }
        acc.into_iter().map(|((r, c), v)| (r, c, v)).collect()
    }

    /// The pi-model branch admittance coefficients `(yff, yft, ytf, ytt)` of branch
    /// `e`, scaled by the switching state so an open (`sw = 0`) branch returns all
    /// zeros:
    ///
    /// ```text
    /// yff = (y + y_fr) / tap²     yft = −y / conj(t)
    /// ytf = −y / t                ytt =  y + y_to
    /// ```
    ///
    /// with `y = g + j b`, `t = tap · e^{j·shift}`, `y_fr = g_fr + j b_fr`,
    /// `y_to = g_to + j b_to`. The one source of this algebra, shared by `ybus`, the
    /// AC flow-operand sensitivity, and its finite-difference test.
    pub(crate) fn branch_admittance(
        &self,
        e: usize,
    ) -> (Complex<f64>, Complex<f64>, Complex<f64>, Complex<f64>) {
        let sw = self.sw[e];
        let y = Complex::new(self.g[e], self.b[e]);
        let tapc = Complex::from_polar(self.tap[e], self.shift[e]);
        let y_fr = Complex::new(self.g_fr[e], self.b_fr[e]);
        let y_to = Complex::new(self.g_to[e], self.b_to[e]);
        let tap2 = self.tap[e] * self.tap[e];
        (
            (y + y_fr) / tap2 * sw,
            -y / tapc.conj() * sw,
            -y / tapc * sw,
            (y + y_to) * sw,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CASE3;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
    }

    #[test]
    #[cfg(feature = "conic")]
    fn typed_instance_objective_and_constraint_masks_reach_the_workspace() {
        use powerio_prob::{ActiveConstraints, ConstraintSelection, Objective};

        let mut network = crate::model::parse_matpower(CASE3).expect("parse case3");
        network.generators_mut()[0].uid = Some("generator-a".into());
        network.branches_mut()[0].uid = Some("line-a".into());
        let mut constraints = ActiveConstraints::default();
        constraints.voltage_bounds = ConstraintSelection::Only(vec!["2".into()]);
        constraints.generator_capability = ConstraintSelection::None;
        constraints.thermal_limits = ConstraintSelection::Only(vec!["line-a".into()]);
        constraints.angle_bounds = ConstraintSelection::None;
        let instance = AcOpfInstance::from_network(network)
            .expect("instance")
            .with_objective(Objective::none())
            .with_constraints(constraints);

        let model = AcNetwork::from_instance(&instance).expect("workspace");
        assert_eq!(model.objective, PreparedObjective::Feasibility);
        assert_eq!(model.voltage_bound_active, vec![false, true, false]);
        assert_eq!(model.generator_capability_active, vec![false, false]);
        assert_eq!(model.thermal_limit_active, vec![true, false, false]);
        assert_eq!(model.angle_bound_active, vec![false, false, false]);
        assert!(model.cq.iter().all(|value| *value == 0.0));
        assert!(model.cl.iter().all(|value| *value == 0.0));
        assert!(model.cc.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn near_zero_impedance_jumper_is_a_tie_not_an_open_circuit() {
        // Regression for the same CATS bug already fixed in `DcNetwork::from_network`
        // (see `model::dc::tests::near_zero_impedance_jumper_is_a_tie_not_an_open_circuit`):
        // a branch with tiny but nonzero impedance (a substation bus-splitting jumper;
        // CaliforniaTestSystem.m has 11, with z2 down to ~1.5e-12) was falling below the
        // old `MIN_Z_SQUARED = 1e-10` guard and getting series `g = b = 0` — treated as an
        // open circuit — instead of the large-but-finite admittance a near-ideal tie
        // actually has. That silently disconnected the two buses in the pi-model, which
        // corrupts both AC power flow (Newton on `ybus`) and the SOCWR relaxation (its Ohm
        // rows read `g`/`b` directly).
        let text = CASE3.replace(
            "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
            "1 3 1e-7 1e-6 0 250 250 250 0 0 1 -360 360;",
        );
        let net = crate::model::parse_matpower(&text).expect("parse jumper case3");
        let ac = AcNetwork::from_network(&net).expect("build AcNetwork with jumper branch");

        let z2 = 1e-7_f64.powi(2) + 1e-6_f64.powi(2);
        let expected_g = 1e-7 / z2;
        let expected_b = -1e-6 / z2;
        approx(ac.g[1], expected_g); // branch index 1 is the 1-3 jumper
        approx(ac.b[1], expected_b);
        assert!(
            ac.b[1].abs() > 1e5,
            "jumper susceptance {} should be large, not the near-zero open-circuit value",
            ac.b[1]
        );
    }

    #[test]
    fn near_zero_jumper_retains_series_admittance_but_suppresses_charging() {
        let text = CASE3.replace(
            "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
            "1 3 1e-7 1e-6 0 250 250 250 0 0 1 -360 360;",
        );
        let mut net = crate::model::parse_matpower(&text).expect("parse jumper case3");
        net.branches_mut()[1].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));
        let ac = AcNetwork::from_network(&net).expect("build charged jumper");
        assert!(ac.g[1].abs() > 1.0e4);
        assert!(ac.b[1].abs() > 1.0e5);
        assert_eq!(
            (ac.g_fr[1], ac.b_fr[1], ac.g_to[1], ac.b_to[1]),
            (0.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn canonical_asymmetric_terminal_charging_reaches_the_pi_model() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse case3");
        net.branches_mut()[0].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));

        let ac = AcNetwork::from_network(&net).expect("build charged AC network");
        approx(ac.g_fr[0], 0.01);
        approx(ac.b_fr[0], 0.02);
        approx(ac.g_to[0], 0.03);
        approx(ac.b_to[0], 0.04);

        let (yff, _, _, ytt) = ac.branch_admittance(0);
        approx(yff.re, ac.g[0] + 0.01);
        approx(yff.im, ac.b[0] + 0.02);
        approx(ytt.re, ac.g[0] + 0.03);
        approx(ytt.im, ac.b[0] + 0.04);
    }

    #[test]
    fn voltage_dependent_loads_fail_closed_but_constant_power_is_explicitly_safe() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse case3");
        net.loads_mut()[0].voltage_model = Some(LoadVoltageModel::ConstantPower);
        AcNetwork::from_network(&net).expect("explicit constant power");

        net.loads_mut()[0].voltage_model = Some(LoadVoltageModel::Zip {
            p_constant_power: net.loads()[0].p,
            q_constant_power: net.loads()[0].q,
            p_constant_current: 0.0,
            q_constant_current: 0.0,
            p_constant_impedance: 0.0,
            q_constant_impedance: 0.0,
            v_nom: Some(1.0),
            load_type: None,
            scaling: None,
        });
        let error = AcNetwork::from_network(&net).unwrap_err();
        assert!(error.contains("voltage-dependent load"), "{error}");

        net.loads_mut()[0].voltage_model = Some(LoadVoltageModel::Exponential {
            p: net.loads()[0].p,
            q: net.loads()[0].q,
            v_nom: Some(1.0),
            gamma_p: 1.0,
            gamma_q: 2.0,
        });
        let error = AcNetwork::from_network(&net).unwrap_err();
        assert!(error.contains("voltage-dependent load"), "{error}");
    }

    #[test]
    fn remote_generator_voltage_control_is_preserved_as_an_acpf_guard() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse case3");
        net.generators_mut()[0].regulated_bus = Some(powerio::BusId(2));
        let ac = AcNetwork::from_network(&net).expect("SOCWR model remains constructible");
        assert!(ac.has_remote_voltage_control);

        net.generators_mut()[0].regulated_bus = None;
        let ac = AcNetwork::from_network(&net).expect("terminal regulation");
        assert!(!ac.has_remote_voltage_control);
    }

    #[test]
    #[cfg(feature = "conic")]
    fn raw_and_normalized_inputs_build_the_same_ac_model() {
        let mut raw = crate::model::parse_matpower(CASE3).expect("parse case3");
        raw.branches_mut()[0].shift = 15.0;
        raw.branches_mut()[0].angmin = -30.0;
        raw.branches_mut()[0].angmax = 30.0;
        raw.branches_mut()[0].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));
        let normalized = raw.to_normalized().expect("normalize case3");
        let a = AcNetwork::from_network(&raw).expect("build raw");
        let b = AcNetwork::from_network(&normalized).expect("build normalized");

        assert_eq!(a.bus_ids, b.bus_ids);
        assert_eq!(a.branch_ids, b.branch_ids);
        assert_eq!(a.gen_ids, b.gen_ids);
        for (left, right) in [
            (&a.g, &b.g),
            (&a.b, &b.b),
            (&a.g_fr, &b.g_fr),
            (&a.b_fr, &b.b_fr),
            (&a.g_to, &b.g_to),
            (&a.b_to, &b.b_to),
            (&a.tap, &b.tap),
            (&a.shift, &b.shift),
            (&a.rate_a, &b.rate_a),
            (&a.angmin, &b.angmin),
            (&a.angmax, &b.angmax),
            (&a.gs, &b.gs),
            (&a.bs, &b.bs),
            (&a.pd, &b.pd),
            (&a.qd, &b.qd),
            (&a.pg, &b.pg),
            (&a.qg, &b.qg),
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
    #[cfg(feature = "conic")]
    fn active_three_winding_transformer_is_lowered_with_stable_ids() {
        let mut net = crate::model::parse_matpower(CASE3).expect("parse case3");
        let mut windings = [1, 2, 3].map(|bus| powerio::Winding::new(powerio::BusId(bus)));
        for winding in &mut windings {
            winding.rate_a = net.base_mva();
        }
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva());
        net.transformers_3w_mut()
            .push(powerio::Transformer3W::new(windings, [impedance; 3]));

        let ac = AcNetwork::from_network(&net).expect("build 3W AC network");
        assert_eq!(ac.n, 4);
        assert_eq!(ac.m, 6);
        assert_eq!(ac.bus_ids, vec![1, 2, 3, 4]);
        assert_eq!(ac.branch_ids, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(&ac.branch_uids[3..], &[None, None, None]);
        for branch in 3..6 {
            assert_eq!(ac.br_to[branch], 3, "winding must terminate at star bus");
            approx(ac.angmin[branch], -std::f64::consts::PI / 3.0);
            approx(ac.angmax[branch], std::f64::consts::PI / 3.0);
        }

        let normalized = net.to_normalized().expect("normalize 3W case");
        let from_normalized = AcNetwork::from_network(&normalized).expect("build normalized 3W");
        assert_eq!(from_normalized.bus_ids, ac.bus_ids);
        assert_eq!(from_normalized.branch_ids, ac.branch_ids);
    }
}
