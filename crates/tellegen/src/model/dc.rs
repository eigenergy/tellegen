//! DC OPF model: the B-theta data — incidence, susceptances, generator bounds,
//! quadratic costs, and line flow limits — built from a powerio `Network`. The
//! gen-cost rescale to per unit happens in `to_normalized`; the angle-bound defaults
//! and the `rate_a == 0` thermal-limit fallback are applied here.

use std::collections::BTreeMap;

use powerio::network::{GenCost, Network};
use powerio::IndexedNetwork;

use super::{normalize_angle_bounds, quadratic_cost_coeffs, reconstruct_ids, Ids};

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
/// `G_inc pg + psh - d = B theta`; branch flows `f = diag(-b .* sw) A theta`.
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
    /// branches; `0` only for a literal zero-impedance record, which cannot be
    /// divided through). A branch with tiny but nonzero impedance — a
    /// substation bus-splitting jumper, common in detailed synthetic cases —
    /// gets a correspondingly large `|b|` rather than being dropped; it is a
    /// real, near-ideal tie, not an open circuit.
    pub b: Vec<f64>,
    /// Branch switching state (1 closed, 0 open). All branches start closed.
    pub sw: Vec<f64>,
    /// Per-unit thermal limit per branch (`rate_a`, with a fallback synthesized
    /// when the source leaves it at 0).
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
    /// System base power (MVA), for recovering MW / $/MWh from per-unit results.
    pub base_mva: f64,
}

impl DcNetwork {
    /// Build the DC OPF model from a parsed powerio `Network`.
    ///
    /// Normalizes through `Network::to_normalized` (per unit, radians, filtered,
    /// densely reindexed, reference inferred), then layers on the solver-prep:
    /// default angle-difference bounds and the `rate_a == 0` thermal-limit fallback.
    pub fn from_network(raw: &Network) -> Result<DcNetwork, String> {
        let norm = raw.to_normalized().map_err(|e| e.to_string())?;
        let view = IndexedNetwork::new(&norm);
        let Ids {
            n,
            m,
            k,
            bus_ids,
            branch_ids,
            gen_ids,
        } = reconstruct_ids(raw, &view)?;

        // Per-bus demand (already per unit on the normalized network).
        let demand = view.pd().to_vec();

        // Branches.
        let branches = view.branches();
        let mut br_from = Vec::with_capacity(m);
        let mut br_to = Vec::with_capacity(m);
        let mut b = Vec::with_capacity(m);
        let mut fmax = Vec::with_capacity(m);
        let mut angmin = Vec::with_capacity(m);
        let mut angmax = Vec::with_capacity(m);
        for br in branches {
            let f = view
                .bus_index(br.from)
                .ok_or_else(|| format!("branch from-bus {} not in index", br.from))?;
            let t = view
                .bus_index(br.to)
                .ok_or_else(|| format!("branch to-bus {} not in index", br.to))?;
            let z2 = br.r * br.r + br.x * br.x;
            // Only a literal zero-impedance record (z2 == 0, undivideable) falls back to 0.
            // A tiny-but-nonzero impedance still gets its true (large) susceptance: it is a
            // real, near-ideal tie (e.g. a substation bus-splitting jumper), not an open
            // circuit, and severing it can wrongly strand generation or reroute flow onto a
            // costlier path. See `tests::near_zero_impedance_jumper_is_a_tie_not_an_open_circuit`.
            let bb = if z2 > 0.0 { -br.x / z2 } else { 0.0 };
            let (amin, amax) = normalize_angle_bounds(br.angmin, br.angmax);
            let rate = if br.rate_a > 0.0 {
                br.rate_a
            } else {
                fallback_rate_a(
                    br.r,
                    br.x,
                    amin,
                    amax,
                    norm.buses[f].vmax,
                    norm.buses[t].vmax,
                )
            };
            br_from.push(f);
            br_to.push(t);
            b.push(bb);
            fmax.push(rate);
            angmin.push(amin);
            angmax.push(amax);
        }
        let sw = vec![1.0; m];

        // Generators.
        let gens = view.generators();
        let mut gen_bus = Vec::with_capacity(k);
        let mut gmax = Vec::with_capacity(k);
        let mut gmin = Vec::with_capacity(k);
        let mut cq = Vec::with_capacity(k);
        let mut cl = Vec::with_capacity(k);
        let mut cc = Vec::with_capacity(k);
        for g in gens {
            let bus = view
                .bus_index(g.bus)
                .ok_or_else(|| format!("generator bus {} not in index", g.bus))?;
            let (q, l, c) = cost_coeffs(g.cost.as_ref())?;
            gen_bus.push(bus);
            gmax.push(g.pmax);
            gmin.push(g.pmin);
            cq.push(q);
            cl.push(l);
            cc.push(c);
        }

        // Shedding cost references the steepest marginal generation cost.
        let marginal_cost_ub = (0..k)
            .map(|i| 2.0 * cq[i] * gmax[i] + cl[i])
            .fold(f64::NEG_INFINITY, f64::max)
            .max(1.0);
        let c_shed = vec![DEFAULT_SHED_COST_MULTIPLIER * marginal_cost_ub; n];

        // First reference bus by dense index. The DC OPF dispatch, flows, and
        // LMPs are invariant to which grounded bus is chosen; only the angle
        // datum shifts, so the exact pick does not matter.
        let ref_bus = *view
            .reference_bus_indices()
            .first()
            .ok_or("normalized network has no reference bus")?;

        Ok(DcNetwork {
            n,
            m,
            k,
            br_from,
            br_to,
            gen_bus,
            b,
            sw,
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
            ref_bus,
            allow_shed: true,
            tau: DEFAULT_TAU,
            bus_ids,
            branch_ids,
            gen_ids,
            base_mva: raw.base_mva,
        })
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
    /// (open / zero-admittance) branches contribute nothing.
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

/// Synthesize a thermal limit for a branch the source left unlimited
/// (`rate_a == 0`), from the bus voltage ceilings and the branch admittance and
/// angle window.
fn fallback_rate_a(r: f64, x: f64, amin: f64, amax: f64, fr_vmax: f64, to_vmax: f64) -> f64 {
    let theta_max = amin.abs().max(amax.abs());
    let zmag = r.hypot(x);
    let ymag = if zmag == 0.0 { 0.0 } else { 1.0 / zmag };
    let cmax =
        (fr_vmax * fr_vmax + to_vmax * to_vmax - 2.0 * fr_vmax * to_vmax * theta_max.cos()).sqrt();
    ymag * fr_vmax.max(to_vmax) * cmax
}

/// Quadratic, linear, and constant cost coefficients `(cq, cl, cc)` for one
/// generator, from its per-unit polynomial cost. Coefficients arrive already
/// rescaled to per unit by `to_normalized`, so this only does the polynomial
/// shaping: drop leading zeros, then read the quadratic, linear, and constant
/// terms. A generator with no cost curve is free (`(0, 0, 0)`). The QP objective
/// only wires in `cq` and `cl` (a constant does not move the argmin); the solve
/// readout adds `sum(cc)` back onto the reported objective, matching how the
/// AC/SOCWR path (`ac::ac_cost_coeffs`) handles the same constant.
fn cost_coeffs(cost: Option<&GenCost>) -> Result<(f64, f64, f64), String> {
    quadratic_cost_coeffs(cost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3, CASE3};

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
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
        // inference, the rate_a fallback, and cost shaping on a full network —
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
