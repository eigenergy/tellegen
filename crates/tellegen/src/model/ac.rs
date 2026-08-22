//! AC network data in the vectorized pi-model admittance form. Carries per-branch
//! series and shunt admittance with transformer tap and phase shift, the per-bus
//! shunt, the real and reactive demand, and the generator injection aggregated to
//! buses — everything the polar AC power flow and its voltage sensitivities read,
//! plus the per-generator bounds and costs the conic OPF optimizes. Built from a
//! powerio `BalancedNetwork` exactly as [`DcNetwork`](super::DcNetwork) is. Gated with the
//! faer paths behind `sensitivity`.

use std::collections::BTreeMap;

use num_complex::Complex;
use powerio::{BalancedNetwork, IndexedNetwork, LoadVoltageModel};
use powerio_prob::{build_ac_opf_instance, AcOpfOptions, Units};

use super::{flatten_gen_costs, normalize_angle_bounds, normalize_for_model, reconstruct_ids, Ids};

const NEAR_ZERO_IMPEDANCE_SQUARED: f64 = 1.0e-10;

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
    /// An active generator names a different regulated bus. The SOCWR model
    /// ignores voltage-control actions; ACPF rejects this explicitly.
    pub has_remote_voltage_control: bool,
}

impl AcNetwork {
    /// Build the AC model from a parsed powerio `BalancedNetwork`, normalizing through
    /// `BalancedNetwork::to_normalized` (per unit, radians, filtered, densely reindexed,
    /// reference inferred) and reading its nodal and generator data from a
    /// `powerio-prob` [`AcOpfInstance`](powerio_prob::AcOpfInstance): per-unit demand,
    /// generator PQ bounds and scheduled output, voltage bands and setpoints. The
    /// cost policy runs first ([`flatten_gen_costs`], so the instance's `GenCost`
    /// accessors accept every row). The instance owns the complete complex pi model,
    /// including per-terminal charging and 3-winding star lowering; Tellegen layers
    /// only its `rate_a == 0` cone sentinel and angle-bound policy on top.
    pub fn from_network(raw: &BalancedNetwork) -> Result<AcNetwork, String> {
        let voltage_dependent_loads = raw
            .loads
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
        let has_remote_voltage_control = raw.generators.iter().any(|generator| {
            generator.in_service
                && generator
                    .regulated_bus
                    .is_some_and(|regulated| regulated != generator.bus)
        });
        let input = normalize_for_model(raw)?;
        let mut norm = input.network;
        let source_rows = input.source_rows;
        flatten_gen_costs(&mut norm)?;
        let view = IndexedNetwork::new(&norm);

        let instance = build_ac_opf_instance(
            &view,
            &AcOpfOptions {
                units: Units::PerUnit,
                // Omitting this row would also omit its topology and terminal
                // charging. Fail closed instead of returning a plausible solve
                // for a different network.
                skip_zero_impedance: false,
                ..AcOpfOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;

        let Ids {
            n,
            m,
            k,
            bus_ids,
            branch_ids,
            gen_ids,
            bus_uids,
            branch_uids,
        } = reconstruct_ids(
            raw,
            &instance.bus_ids,
            &instance.branches.source_rows,
            &instance.generators.source_rows,
            &source_rows,
        )?;

        let mut vm_set = instance.vm_setpoints();
        let slack = instance
            .reference_buses
            .single()
            .map_err(|e| e.to_string())?;

        // Move the complete PowerIO problem columns out of the one-shot instance.
        // Its bus shunts already include folded self-loop pi stamps, and its active
        // branch columns carry canonical per-terminal charging.
        let buses = instance.buses;
        let generators = instance.generators;
        let branches = instance.branches;
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
        let rate_a = branches
            .s_max
            .into_iter()
            .map(|rate| if rate > 0.0 { rate } else { 1.0e3 })
            .collect();
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
        let gen_bus = generators.bus_of_gen;
        for (i, bus) in gen_bus.iter().copied().enumerate() {
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
        let cq = generators.q.into_iter().map(|value| value / 2.0).collect();
        let cl = generators.c;
        let cc = generators.c0;
        let pmin = generators.pmin;
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
    }

    #[test]
    fn near_zero_jumper_retains_series_admittance_but_suppresses_charging() {
        let text = CASE3.replace(
            "1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
            "1 3 1e-7 1e-6 0 250 250 250 0 0 1 -360 360;",
        );
        let mut net = powerio::parse_str(&text, "matpower")
            .expect("parse jumper case3")
            .network;
        net.branches[1].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));
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
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse case3")
            .network;
        net.branches[0].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));

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
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse case3")
            .network;
        net.loads[0].voltage_model = Some(LoadVoltageModel::ConstantPower);
        AcNetwork::from_network(&net).expect("explicit constant power");

        net.loads[0].voltage_model = Some(LoadVoltageModel::Zip {
            p_constant_power: net.loads[0].p,
            q_constant_power: net.loads[0].q,
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

        net.loads[0].voltage_model = Some(LoadVoltageModel::Exponential {
            p: net.loads[0].p,
            q: net.loads[0].q,
            v_nom: Some(1.0),
            gamma_p: 1.0,
            gamma_q: 2.0,
        });
        let error = AcNetwork::from_network(&net).unwrap_err();
        assert!(error.contains("voltage-dependent load"), "{error}");
    }

    #[test]
    fn remote_generator_voltage_control_is_preserved_as_an_acpf_guard() {
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse case3")
            .network;
        net.generators[0].regulated_bus = Some(powerio::BusId(2));
        let ac = AcNetwork::from_network(&net).expect("SOCWR model remains constructible");
        assert!(ac.has_remote_voltage_control);

        net.generators[0].regulated_bus = None;
        let ac = AcNetwork::from_network(&net).expect("terminal regulation");
        assert!(!ac.has_remote_voltage_control);
    }

    #[test]
    fn raw_and_normalized_inputs_build_the_same_ac_model() {
        let mut raw = powerio::parse_str(CASE3, "matpower")
            .expect("parse case3")
            .network;
        raw.branches[0].shift = 15.0;
        raw.branches[0].angmin = -30.0;
        raw.branches[0].angmax = 30.0;
        raw.branches[0].charging = Some(powerio::BranchCharging::new(0.01, 0.02, 0.03, 0.04));
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
    fn active_three_winding_transformer_is_lowered_with_stable_ids() {
        let mut net = powerio::parse_str(CASE3, "matpower")
            .expect("parse case3")
            .network;
        let mut windings = [1, 2, 3].map(|bus| powerio::Winding::new(powerio::BusId(bus)));
        for winding in &mut windings {
            winding.rate_a = net.base_mva;
        }
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva);
        net.transformers_3w
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
