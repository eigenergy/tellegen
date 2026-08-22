//! DC OPF model: the B-theta data — incidence, susceptances, generator bounds,
//! quadratic costs, and line flow limits — built from a powerio `BalancedNetwork`. The
//! generation cost rescale to per unit happens in `to_normalized`; Tellegen applies
//! its angle defaults before ordinary limits are synthesized and before lowered
//! three-winding limits are resynthesized.

use std::collections::BTreeMap;

use powerio::BalancedNetwork;
use powerio::{DcConvention, IndexedNetwork};
use powerio_prob::{build_dc_opf_instance, DcOpfOptions, Units};

use super::{
    branch_ids_for_view_rows, bus_ids_for_source_rows, flatten_gen_costs, ids_for_view_rows,
    normalize_angle_bounds, normalize_for_model, project_source_rows, uids_for_source_rows,
};

/// Strong-convexity regularization on the flows.
const DEFAULT_TAU: f64 = 1e-2;

/// Load-shedding cost = multiplier x peak marginal generation cost, so the
/// solver only sheds when capacity or the network physically cannot serve the
/// load.
const DEFAULT_SHED_COST_MULTIPLIER: f64 = 10.0;

/// B-theta DC OPF model data. Indices are dense `[0, n)` / `[0, m)` / `[0, k)`
/// over the normalized network; `bus_ids`, `branch_ids`, and `gen_ids` map dense
/// indices back to source ids for output payloads.
///
/// Susceptance-weighted Laplacian `B = A' diag(-b .* sw) A`; DC power balance
/// `G_inc pg + psh - fixed_withdrawal(sw) = B theta`; branch flows
/// `f = diag(-b .* sw) A theta + sw .* flow_offset`.
#[derive(Clone)]
pub struct DcNetwork {
    /// Buses, branches, generators after filtering (in-service, non-isolated).
    pub n: usize,
    pub m: usize,
    pub k: usize,
    /// Branch endpoints in dense bus-index space (the rows of the incidence
    /// matrix `A`: `+1` at `from`, `-1` at `to`).
    pub br_from: Vec<usize>,
    pub br_to: Vec<usize>,
    /// Bus each generator injects at, dense index.
    pub gen_bus: Vec<usize>,
    /// Branch susceptance `b = -x / (r^2 + x^2)` (negative for inductive
    /// branches). A branch with tiny but nonzero impedance — a
    /// substation bus-splitting jumper, common in detailed synthetic cases —
    /// gets a correspondingly large `|b|` rather than being dropped; it is a
    /// real, near-ideal tie, not an open circuit.
    pub b: Vec<f64>,
    /// Branch switching state (1 closed, 0 open). All branches start closed.
    pub sw: Vec<f64>,
    /// Branch phase shifts in radians, in the active branch order from the
    /// powerio problem instance.
    pub shift: Vec<f64>,
    /// Fixed affine branch flow term `-b_powerio .* shift`. The term is
    /// multiplied by [`Self::sw`] when a branch status varies.
    pub flow_offset: Vec<f64>,
    /// Per unit thermal limit per branch (`rate_a`, with a fallback synthesized
    /// against the normalized angle window when the source leaves it at 0).
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
    /// Load-shedding penalty per bus.
    pub c_shed: Vec<f64>,
    /// Per-unit active demand per bus.
    pub demand: Vec<f64>,
    /// Per-unit shunt conductance withdrawal per bus.
    pub shunt_conductance: Vec<f64>,
    /// Nodal phase shift withdrawal with every active source branch closed.
    pub p_shift: Vec<f64>,
    /// The complete all-closed withdrawal reported by
    /// `DcOpfInstance::fixed_nodal_withdrawal`.
    pub fixed_withdrawal: Vec<f64>,
    /// Reference (slack) bus, dense index.
    pub ref_bus: usize,
    /// Whether load shedding is permitted. When `false`, the shedding variables are
    /// pinned to zero, so a case that cannot serve its load reports infeasible (matching
    /// the published PGLib behavior) instead of shedding. The solve edge sets this from
    /// the request; built models default to `true` so a direct solve degrades gracefully.
    pub allow_shed: bool,
    /// Flow regularization parameter.
    pub tau: f64,
    /// Dense bus index -> original source bus id.
    pub bus_ids: Vec<usize>,
    /// Dense branch index -> original source branch id.
    pub branch_ids: Vec<usize>,
    /// Dense generator index -> original source generator id.
    pub gen_ids: Vec<usize>,
    /// Dense branch index -> original source branch row. `None` marks a branch
    /// synthesized while lowering a source element.
    pub branch_source_rows: Vec<Option<usize>>,
    /// Dense generator index -> original source generator row.
    pub gen_source_rows: Vec<Option<usize>>,
    /// Dense bus index -> powerio row uid (`None` when the source network carried
    /// no uids). Lets edits address a bus as `"buses:0"` and responses echo the uid.
    pub bus_uids: Vec<Option<String>>,
    /// Dense branch index -> powerio row uid; the branch counterpart of `bus_uids`.
    pub branch_uids: Vec<Option<String>>,
    /// System base power (MVA), for recovering MW / $/MWh from per-unit results.
    pub base_mva: f64,
}

impl DcNetwork {
    /// Build the DC OPF model from a parsed powerio `BalancedNetwork`.
    ///
    /// Normalizes through `BalancedNetwork::to_normalized` (per unit, radians, filtered,
    /// densely reindexed, reference inferred), builds a `powerio-prob`
    /// [`DcOpfInstance`](powerio_prob::DcOpfInstance) as the owner of the nodal and
    /// generator interpretation (per unit withdrawal, branch phase terms, generator
    /// PQ bounds, source rows, synthesized thermal limits, and reference coverage),
    /// then applies the cost policy ([`flatten_gen_costs`], run before the instance so
    /// its `GenCost` accessors accept every row). Tellegen's documented +-60 degree
    /// default is applied to source branches before the instance synthesizes an
    /// unrated limit, and again to rows created by three-winding lowering.
    pub fn from_network(raw: &BalancedNetwork) -> Result<DcNetwork, String> {
        let input = normalize_for_model(raw)?;
        let mut norm = input.network;
        let source_rows = input.source_rows;
        // Apply the policy before PowerIO synthesizes thermal limits. Doing this
        // directly avoids allocating normalization diagnostics for every ordinary
        // MATPOWER branch whose source uses the typical +-360 spelling.
        for branch in &mut norm.branches {
            (branch.angmin, branch.angmax) = normalize_angle_bounds(branch.angmin, branch.angmax);
        }
        // Cost policy as a `BalancedNetwork` pre-pass: fit piecewise / strip artifacts / treat
        // a missing cost as free, writing plain quadratics the instance builder reads.
        flatten_gen_costs(&mut norm)?;
        let view = IndexedNetwork::new(&norm);

        // `SeriesImpedance` is the same `x/(r^2+x^2)` tellegen negates below, so the
        // instance and the B-theta model now weight a branch alike. `PerUnit` is inert:
        // the normalized network is already per unit (`per_unit_base() == 1`). The
        // instance owns the generator PQ bounds, nodal demand, and reference coverage.
        let instance = build_dc_opf_instance(
            &view,
            &DcOpfOptions {
                convention: DcConvention::SeriesImpedance,
                units: Units::PerUnit,
                skip_zero_impedance: false,
                synthesize_unrated_limits: true,
            },
        )
        .map_err(|e| e.to_string())?;

        let n = instance.n_buses;
        let m = instance.n_branches();
        let k = instance.n_generators();

        if source_rows.buses.len() != n {
            return Err(format!(
                "bus provenance length {} != problem bus count {n}",
                source_rows.buses.len()
            ));
        }
        let bus_ids =
            bus_ids_for_source_rows(&source_rows.buses, &source_rows.transformers_3w, raw)?;
        let bus_uids = uids_for_source_rows(&source_rows.buses, &raw.buses, |bus| &bus.uid, "bus")?;
        let branch_source_rows = project_source_rows(
            &instance.branches.source_rows,
            &source_rows.branches,
            "branch",
        )?;
        let gen_source_rows = project_source_rows(
            &instance.generators.source_rows,
            &source_rows.generators,
            "generator",
        )?;
        let branch_ids = branch_ids_for_view_rows(
            &instance.branches.source_rows,
            &source_rows.branches,
            &source_rows.transformers_3w,
            raw,
        )?;
        let gen_ids = ids_for_view_rows(
            &instance.generators.source_rows,
            &source_rows.generators,
            raw.generators.len(),
            "generator",
        )?;
        let branch_uids = uids_for_source_rows(
            &branch_source_rows,
            &raw.branches,
            |branch| &branch.uid,
            "branch",
        )?;

        // Per-bus demand and reference, moved straight out of the freshly built,
        // locally owned instance: from_network runs per Study commit and preview, so
        // this stays clone free. `single()` rather than a first element: `DcNetwork`
        // grounds one bus, so several references means several islands and every island
        // past the first would stay singular.
        let ref_bus = instance
            .reference_buses
            .single()
            .map_err(|e| e.to_string())?;
        let fixed_withdrawal = instance.fixed_nodal_withdrawal();
        let flow_offset = instance.branch_flow_offset();
        let demand = instance.p_d.clone();
        let shunt_conductance = instance.g_s.clone();
        let p_shift = instance.p_shift.clone();

        // Tellegen historically stores the negative of powerio's positive
        // inductive susceptance. Keep that internal sign while taking every
        // branch column and affine term from the same instance.
        let br_from = instance.branches.from_bus.clone();
        let br_to = instance.branches.to_bus.clone();
        let b = instance.branches.b.iter().map(|value| -value).collect();
        let shift = instance.branches.shift.clone();
        let (angmin, angmax): (Vec<_>, Vec<_>) = instance
            .branches
            .angle_min
            .iter()
            .copied()
            .zip(instance.branches.angle_max.iter().copied())
            .map(|(min, max)| normalize_angle_bounds(min, max))
            .unzip();
        // Source branches were clamped above. Three-winding transformers are lowered
        // afterwards, so their synthetic branches still arrive with the unconstrained
        // spelling. Recompute every unrated limit from the final stored window. This is
        // idempotent for source branches and keeps the same policy for lowered rows
        // without reaching into PowerIO's private lowering.
        let fmax = instance
            .branches
            .f_max
            .iter()
            .enumerate()
            .map(|(index, &limit)| {
                let row = instance.branches.source_rows[index];
                let branch = &view.network().branches[row];
                if branch.rate_a <= 0.0 {
                    let from = &view.network().buses[instance.branches.from_bus[index]];
                    let to = &view.network().buses[instance.branches.to_bus[index]];
                    let window = angmin[index].abs().max(angmax[index].abs());
                    branch.synthesize_rate_a(window, (from.vmin, from.vmax), (to.vmin, to.vmax))
                } else {
                    limit
                }
            })
            .collect();
        let sw = vec![1.0; m];

        // powerio states the objective as `0.5*q*p^2 + c*p + c0`; Tellegen's
        // stored `cq` is the coefficient of `p^2` before its solver writes `2*cq`
        // on the Hessian.
        let gen_bus = instance.generators.bus_of_gen.clone();
        let cq: Vec<f64> = instance
            .generators
            .q
            .iter()
            .map(|value| value / 2.0)
            .collect();
        let cl = instance.generators.c.clone();
        let cc = instance.generators.c0.clone();
        let gmax = instance.generators.pmax.clone();
        let gmin = instance.generators.pmin.clone();

        // Shedding cost references the steepest marginal generation cost.
        let marginal_cost_ub = (0..k)
            .map(|i| 2.0 * cq[i] * gmax[i] + cl[i])
            .fold(f64::NEG_INFINITY, f64::max)
            .max(1.0);
        let c_shed = vec![DEFAULT_SHED_COST_MULTIPLIER * marginal_cost_ub; n];

        Ok(DcNetwork {
            n,
            m,
            k,
            br_from,
            br_to,
            gen_bus,
            b,
            sw,
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
            c_shed,
            demand,
            shunt_conductance,
            p_shift,
            fixed_withdrawal,
            ref_bus,
            allow_shed: true,
            tau: DEFAULT_TAU,
            bus_ids,
            branch_ids,
            gen_ids,
            branch_source_rows,
            gen_source_rows,
            bus_uids,
            branch_uids,
            base_mva: raw.base_mva,
        })
    }

    /// Phase shift withdrawal for the current branch statuses. A phase shifter
    /// on an open branch contributes neither a branch flow offset nor a nodal
    /// withdrawal.
    pub fn phase_withdrawal(&self) -> Vec<f64> {
        let mut withdrawal = vec![0.0; self.n];
        for e in 0..self.m {
            let offset = self.sw[e] * self.flow_offset[e];
            withdrawal[self.br_from[e]] += offset;
            withdrawal[self.br_to[e]] -= offset;
        }
        withdrawal
    }

    /// Fixed nodal withdrawal for the current demand and branch statuses.
    pub fn current_fixed_withdrawal(&self) -> Vec<f64> {
        let phase = self.phase_withdrawal();
        (0..self.n)
            .map(|i| self.demand[i] + self.shunt_conductance[i] + phase[i])
            .collect()
    }

    /// Complete affine branch flow offset at the current status.
    pub fn current_flow_offset(&self, branch: usize) -> f64 {
        self.sw[branch] * self.flow_offset[branch]
    }

    /// The shedding upper bound at bus `i`: the curtailable load `max(d, 0)` when
    /// shedding is permitted, else `0` (the variable is pinned to zero). The one source
    /// of this rule — the DC OPF solve bounds `psh` by it, and the KKT sensitivity
    /// derivation reads the same cap so its linearization matches the solved program.
    pub fn shed_cap(&self, i: usize) -> f64 {
        if self.allow_shed {
            self.demand[i].max(0.0)
        } else {
            0.0
        }
    }

    /// Susceptance-weighted Laplacian `B = A' diag(-b .* sw) A` as summed,
    /// deduplicated `(row, col, value)` triplets in `(row, col)` order. Parallel
    /// branches between the same pair of buses are accumulated. Zero-weight
    /// (open) branches contribute nothing.
    #[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
    pub fn susceptance_coo(&self) -> Vec<(usize, usize, f64)> {
        let mut acc: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for e in 0..self.m {
            let w = -self.b[e] * self.sw[e];
            if w == 0.0 {
                continue;
            }
            let (i, j) = (self.br_from[e], self.br_to[e]);
            *acc.entry((i, i)).or_default() += w;
            *acc.entry((j, j)).or_default() += w;
            *acc.entry((i, j)).or_default() -= w;
            *acc.entry((j, i)).or_default() -= w;
        }
        acc.into_iter().map(|((r, c), v)| (r, c, v)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3, CASE3, DEFAULT_ANGLE_BOUND_PAD};

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
        let net = powerio::parse_str(&case, "matpower")
            .expect("parse")
            .network;
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

    #[test]
    fn unconstrained_unrated_branches_use_the_sixty_degree_window() {
        for (amin, amax) in [(-360, 360), (0, 0)] {
            let case = CASE3.replace(
                " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
                &format!(" 1 2 0.01 0.1 0 0 0 0 0 0 1 {amin} {amax};"),
            );
            let net = powerio::parse_str(&case, "matpower")
                .expect("parse")
                .network;
            let dc = DcNetwork::from_network(&net).expect("build");

            approx(dc.angmin[0], -DEFAULT_ANGLE_BOUND_PAD);
            approx(dc.angmax[0], DEFAULT_ANGLE_BOUND_PAD);
            approx(dc.fmax[0], 1.1 * 1.1 / 0.01_f64.hypot(0.1));
            approx(dc.fmax[1], 2.5);
            approx(dc.fmax[2], 2.5);
        }
    }

    #[test]
    fn lowered_three_winding_branches_use_the_sixty_degree_window() {
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse")
            .network;
        let mut windings = [1, 2, 3].map(|bus| powerio::Winding::new(powerio::BusId(bus)));
        windings[0].rate_a = net.base_mva;
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva);
        net.transformers_3w
            .push(powerio::Transformer3W::new(windings, [impedance; 3]));

        let dc = DcNetwork::from_network(&net).expect("build");
        assert_eq!(dc.m, 6);
        approx(dc.fmax[3], 1.0);
        for branch in 3..6 {
            assert_eq!(dc.branch_source_rows[branch], None);
            approx(dc.angmin[branch], -DEFAULT_ANGLE_BOUND_PAD);
            approx(dc.angmax[branch], DEFAULT_ANGLE_BOUND_PAD);
            if branch > 3 {
                // Equal pairwise impedances lower to r=0.01, x=0.1 on each winding.
                approx(dc.fmax[branch], 1.1 * 1.1 / 0.01_f64.hypot(0.1));
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
        let raw = powerio::parse_str(&text, "matpower")
            .expect("parse affine case")
            .network;
        let normalized = raw.to_normalized().expect("normalize affine case");
        let a = DcNetwork::from_network(&raw).expect("build raw");
        let b = DcNetwork::from_network(&normalized).expect("build normalized");

        assert_eq!(a.bus_ids, b.bus_ids);
        assert_eq!(a.branch_ids, b.branch_ids);
        assert_eq!(a.gen_ids, b.gen_ids);
        assert_eq!(a.branch_source_rows, b.branch_source_rows);
        assert_eq!(a.gen_source_rows, b.gen_source_rows);
        for (left, right) in [
            (&a.demand, &b.demand),
            (&a.shunt_conductance, &b.shunt_conductance),
            (&a.p_shift, &b.p_shift),
            (&a.b, &b.b),
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
        assert_eq!(dc.gen_ids, vec![1, 2]);
        assert_eq!(dc.branch_source_rows, vec![Some(0), Some(1), Some(2)]);
        assert_eq!(dc.gen_source_rows, vec![Some(0), Some(1)]);
        approx(dc.base_mva, 100.0);
        // Bus 1 is the MATPOWER slack (type 3) -> dense index 0.
        assert_eq!(dc.ref_bus, 0);
    }

    #[test]
    fn ids_remain_source_order_after_filtering() {
        let mut net = powerio::parse_str(crate::model::CASE3, "matpower")
            .expect("parse case3")
            .network;
        net.branches[0].in_service = false;
        net.generators[0].in_service = false;

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
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse affine case")
            .network;
        let mut dc = DcNetwork::from_network(&net).expect("build affine case");

        approx(dc.demand[1], 0.9);
        approx(dc.shunt_conductance[1], 0.1);
        approx(dc.flow_offset[0], dc.b[0] * dc.shift[0]);
        approx(dc.p_shift.iter().sum(), 0.0);
        for i in 0..dc.n {
            approx(dc.current_fixed_withdrawal()[i], dc.fixed_withdrawal[i]);
        }
        assert!(dc.fmax[0].is_finite() && dc.fmax[0] > 0.0);

        // Opening the phase-shifting branch removes both affine terms. Demand
        // and shunt withdrawal remain at bus 2.
        dc.sw[0] = 0.0;
        approx(dc.current_flow_offset(0), 0.0);
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
        for (r, c, v) in dc.susceptance_coo() {
            dense[r][c] = v;
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
        approx(dc.demand[0], 0.0);
        approx(dc.demand[1], 0.9);
        approx(dc.demand[2], 0.0);
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
    fn piecewise_costs_project_to_quadratic() {
        let text = CASE3
            .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 1 100 3 200 7;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 100 50;");
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse piecewise case3")
            .network;
        let dc = DcNetwork::from_network(&net).expect("build piecewise DcNetwork");
        approx(dc.cq[0], 1.0);
        approx(dc.cl[0], 1.0);
        approx(dc.cq[1], 0.0);
        approx(dc.cl[1], 50.0);
    }

    #[test]
    fn angle_bounds_default_to_sixty_degrees() {
        let dc = parse_case3();
        // The +-360 degree MATPOWER default collapses to the +-60 degree window.
        let pad = 60.0_f64.to_radians();
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
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse ACTIVSg200")
            .network;
        let dc = DcNetwork::from_network(&net).expect("build DcNetwork from ACTIVSg200");

        assert!(dc.n > 0 && dc.m > 0 && dc.k > 0);
        assert!(dc.ref_bus < dc.n);
        assert_eq!(dc.bus_ids.len(), dc.n);
        assert_eq!(dc.demand.len(), dc.n);
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
        for &d in &dc.demand {
            assert!(d.is_finite());
        }
        // B is a grounded Laplacian regardless of connectivity: rows sum to zero.
        let mut row_sum = vec![0.0f64; dc.n];
        for (r, _c, v) in dc.susceptance_coo() {
            row_sum[r] += v;
        }
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
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse jumper case3")
            .network;
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
