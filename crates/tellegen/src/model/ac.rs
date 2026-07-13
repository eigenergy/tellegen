//! AC network data in the vectorized pi-model admittance form. Carries per-branch
//! series and shunt admittance with transformer tap and phase shift, the per-bus
//! shunt, the real and reactive demand, and the generator injection aggregated to
//! buses — everything the polar AC power flow and its voltage sensitivities read,
//! plus the per-generator bounds and costs the conic OPF optimizes. Built from a
//! powerio `Network` exactly as [`DcNetwork`](super::DcNetwork) is. Gated with the
//! faer paths behind `sensitivity`.

use std::collections::BTreeMap;

use num_complex::Complex;
use powerio::network::Network;
use powerio::IndexedNetwork;
use powerio_prob::{build_ac_opf_instance, AcOpfOptions, Units};

use super::{flatten_gen_costs, normalize_angle_bounds, reconstruct_ids, Ids, MIN_Z_SQUARED};

/// AC network data in the vectorized pi-model admittance form.
///
/// Each branch contributes the standard pi-model stamp built from its series
/// admittance `y = g + j b`, the complex tap `t = tap · e^{j·shift}`, and the
/// from/to line-charging shunts; [`AcNetwork::ybus`] assembles the bus admittance
/// matrix `Y`. The net injection at bus `i` is `S_i = V_i · conj((Y V)_i)`, equal
/// to `(pg_i − pd_i) + j(qg_i − qd_i)`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AcNetwork {
    /// Buses and branches after filtering (in-service, non-isolated).
    pub n: usize,
    pub m: usize,
    /// Branch endpoints in dense bus-index space.
    pub br_from: Vec<usize>,
    pub br_to: Vec<usize>,
    /// Series conductance `g = r/(r²+x²)` and susceptance `b = −x/(r²+x²)` per
    /// branch (`0` only for a literal zero-impedance record; a tiny but nonzero
    /// impedance still gets its true, correspondingly large admittance).
    pub g: Vec<f64>,
    pub b: Vec<f64>,
    /// From/to-side shunt admittance (line charging). A MATPOWER source splits the
    /// single branch charging `br_b` evenly (`b_fr = b_to = br_b/2`) with no
    /// charging conductance (`g_fr = g_to = 0`).
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
    pub rate_a: Vec<f64>,
    /// Per-branch voltage-angle-difference bounds (radians): `va_from − va_to ∈
    /// [angmin, angmax]`. Normalized to the ±60° MATPOWER/PowerModels convention when
    /// the source leaves them unset or unconstrained (shared with the DC model). The AC
    /// OPF enforces these as linear inequalities; the conic SOCWR maps them onto the
    /// W-space products `wr`/`wi`.
    pub angmin: Vec<f64>,
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
    pub pmin: Vec<f64>,
    pub pmax: Vec<f64>,
    pub qmin: Vec<f64>,
    pub qmax: Vec<f64>,
    /// Per-unit generation cost `cq[g] pg² + cl[g] pg + cc[g]` per generator.
    pub cq: Vec<f64>,
    pub cl: Vec<f64>,
    pub cc: Vec<f64>,
    /// Dense generator index -> original source generator id.
    pub gen_ids: Vec<usize>,
    /// Per-bus voltage magnitude bounds, and the per-bus magnitude setpoint: the
    /// regulating generator's voltage setpoint (`vg`) at PV and slack buses, the bus
    /// voltage elsewhere. The power flow holds PV/slack magnitudes at this value; it also
    /// seeds the flat start.
    pub vm_min: Vec<f64>,
    pub vm_max: Vec<f64>,
    pub vm_set: Vec<f64>,
    /// Reference (slack) bus, dense index.
    pub slack: usize,
    /// Dense index -> original source id, as in [`DcNetwork`](super::DcNetwork).
    pub bus_ids: Vec<usize>,
    pub branch_ids: Vec<usize>,
    /// Dense index -> powerio row uid (`None` when the source network carried no
    /// uids), as in [`DcNetwork`](super::DcNetwork).
    pub bus_uids: Vec<Option<String>>,
    pub branch_uids: Vec<Option<String>>,
    /// System base power (MVA).
    pub base_mva: f64,
}

impl AcNetwork {
    /// Build the AC model from a parsed powerio `Network`, normalizing through
    /// `Network::to_normalized` (per unit, radians, filtered, densely reindexed,
    /// reference inferred) and reading its nodal and generator data from a
    /// `powerio-prob` [`AcOpfInstance`](powerio_prob::AcOpfInstance): per-unit demand,
    /// generator PQ bounds and scheduled output, voltage bands and setpoints. The
    /// cost policy runs first ([`flatten_gen_costs`], so the instance's `GenCost`
    /// accessors accept every row); the complex pi-model admittance, the line-charging
    /// drop, the `rate_a == 0` cone sentinel, and the angle-bound normalization are
    /// layered on from the `IndexedNetwork` afterward.
    pub fn from_network(raw: &Network) -> Result<AcNetwork, String> {
        let mut norm = raw.to_normalized().map_err(|e| e.to_string())?;
        let costs = flatten_gen_costs(&mut norm)?;
        let view = IndexedNetwork::new(&norm);
        let Ids {
            n,
            m,
            k,
            bus_ids,
            branch_ids,
            gen_ids,
            bus_uids,
            branch_uids,
        } = reconstruct_ids(raw, &view)?;

        let instance = build_ac_opf_instance(
            &view,
            &AcOpfOptions {
                units: Units::PerUnit,
                skip_zero_impedance: true,
            },
        )
        .map_err(|e| e.to_string())?;

        // Nodal demand moved out of the instance (from_network runs per Study commit and
        // preview, and the instance is freshly built and owned here).
        let pd = instance.buses.p_d;
        let qd = instance.buses.q_d;
        // Bus shunts stay on the `IndexedNetwork` aggregates: the instance folds a
        // self-loop branch's pi-model stamp into `g_s`/`b_s`, but tellegen keeps every
        // source branch as a dense column (its `ybus` stamps the self-loop onto the
        // diagonal), so the folded form would double-count it here.
        let gs = view.gs().to_vec();
        let bs = view.bs().to_vec();

        let branches = view.branches();
        let mut br_from = Vec::with_capacity(m);
        let mut br_to = Vec::with_capacity(m);
        let mut g = Vec::with_capacity(m);
        let mut b = Vec::with_capacity(m);
        let mut g_fr = Vec::with_capacity(m);
        let mut b_fr = Vec::with_capacity(m);
        let mut g_to = Vec::with_capacity(m);
        let mut b_to = Vec::with_capacity(m);
        let mut tap = Vec::with_capacity(m);
        let mut shift = Vec::with_capacity(m);
        let mut rate_a = Vec::with_capacity(m);
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
            // A tiny-but-nonzero impedance still gets its true (large) series admittance: it
            // is a real, near-ideal tie (e.g. a substation bus-splitting jumper), not an open
            // circuit, and severing it can wrongly strand generation or reactive support (the
            // same bug fixed for DC in `DcNetwork::from_network`; see
            // `tests::near_zero_impedance_jumper_is_a_tie_not_an_open_circuit`). The line
            // charging on such a jumper is still dropped below, unrelated to this guard.
            let (gg, bb) = if z2 > 0.0 {
                (br.r / z2, -br.x / z2)
            } else {
                (0.0, 0.0)
            };
            br_from.push(f);
            br_to.push(t);
            g.push(gg);
            b.push(bb);
            // MATPOWER charging: split evenly, no charging conductance. This guard is
            // independent of the series-admittance one above: a near-ideal tie (z2 below
            // MIN_Z_SQUARED but not literally zero) now carries its true, large series g/b,
            // so it is no longer a dangling bus in the series terms; but its charging, which
            // scales with a real line's length, is still dropped, since it is negligibly
            // small next to that series admittance and keeping it on a literal zero-impedance
            // jumper (z2 == 0, g = b = 0, no series coupling at all) over-determines the
            // reactive balance at an isolated zero-injection bus (its only reactive terms
            // would be the two charging shunts), forcing w → 0 against the voltage floor — a
            // spurious SOCWR/AC-OPF infeasibility fixed by PR #13. (powerio 0.3.3 still emits
            // the charging here.)
            let charging = if z2 > MIN_Z_SQUARED { br.b / 2.0 } else { 0.0 };
            g_fr.push(0.0);
            b_fr.push(charging);
            g_to.push(0.0);
            b_to.push(charging);
            // to_normalized already maps 0 -> 1 via effective_tap; guard anyway.
            tap.push(if br.tap == 0.0 { 1.0 } else { br.tap });
            shift.push(br.shift);
            // rate_a == 0 means unlimited; a large sentinel keeps the cone uniform.
            rate_a.push(if br.rate_a > 0.0 { br.rate_a } else { 1.0e3 });
            let (amin, amax) = normalize_angle_bounds(br.angmin, br.angmax);
            angmin.push(amin);
            angmax.push(amax);
        }
        let sw = vec![1.0; m];

        // Voltage-magnitude bounds and the per-bus magnitude setpoint, from the instance.
        // The setpoint starts from the case voltage (bus.vm, or 1.0 when unset) and is
        // overwritten below by each in-service generator's voltage setpoint (vg) at its
        // bus, so PV and slack buses regulate to the generator setpoint (last-wins per bus,
        // as MATPOWER does), not the bus.vm guess.
        let mut vm_set: Vec<f64> = instance
            .buses
            .vm
            .iter()
            .map(|&v| if v > 0.0 { v } else { 1.0 })
            .collect();
        let vm_min = instance.buses.vm_min;
        let vm_max = instance.buses.vm_max;

        // Per-bus aggregates (power flow operating point) and per-generator data (the
        // conic OPF decision variables) from the instance; cost from the pre-pass. The
        // instance's generator columns follow the normalized generator order, the same
        // order `flatten_gen_costs` returns, so `costs[i]` pairs with column `i`.
        let mut pg = vec![0.0; n];
        let mut qg = vec![0.0; n];
        let gen_bus = instance.generators.bus_of_gen;
        let pmin = instance.generators.pmin;
        let pmax = instance.generators.pmax;
        let qmin = instance.generators.qmin;
        let qmax = instance.generators.qmax;
        let gen_pg = instance.generators.pg;
        let gen_qg = instance.generators.qg;
        let gen_vg = instance.generators.vg;
        let mut cq = Vec::with_capacity(k);
        let mut cl = Vec::with_capacity(k);
        let mut cc = Vec::with_capacity(k);
        for i in 0..k {
            let bus = gen_bus[i];
            pg[bus] += gen_pg[i];
            qg[bus] += gen_qg[i];
            // Regulate this bus's magnitude to the generator's voltage setpoint, clamped
            // into the bus magnitude band: the power flow holds PV/slack magnitudes at
            // `vm_set` with no `vm` column to bound them, so an out-of-band `vg` would pin
            // the bus at an infeasible magnitude and the sensitivity would linearize there.
            let vg = gen_vg[i];
            if vg > 0.0 {
                vm_set[bus] = vg.clamp(vm_min[bus], vm_max[bus]);
            }
            let (q, l, c) = costs[i];
            cq.push(q);
            cl.push(l);
            cc.push(c);
        }

        let slack = *instance
            .reference_buses
            .first()
            .ok_or("normalized network has no reference bus")?;

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
            rate_a,
            angmin,
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
            pmin,
            pmax,
            qmin,
            qmax,
            cq,
            cl,
            cc,
            gen_ids,
            vm_min,
            vm_max,
            vm_set,
            slack,
            bus_ids,
            branch_ids,
            bus_uids,
            branch_uids,
            base_mva: raw.base_mva,
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
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse jumper case3")
            .network;
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
        // CASE3's branches all carry zero charging (`br.b == 0`), so this fixture can't
        // pin the separate charging guard (still keyed to MIN_Z_SQUARED, unaffected by
        // this fix, and covered by
        // `problem::conic::tests::zero_impedance_jumper_to_dangling_bus_stays_feasible`).
    }
}
