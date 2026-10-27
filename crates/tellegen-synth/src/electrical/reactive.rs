//! Reactive power planning. Reactive power does not travel far: moving it
//! over a loaded line costs a voltage magnitude gradient and adds `I²X`
//! losses, so transmission planners supply it where it is consumed, with
//! capacitor banks at load substations and shunt reactors where lightly
//! loaded high voltage lines charge the system (Kundur, *Power System
//! Stability and Control*, 1994, §11.2; Taylor, *Power System Voltage
//! Stability*, 1994).
//!
//! The plan holds a flat voltage profile at the generator set point
//! ([`GENERATOR_VG`]), so no reactive power is scheduled across a branch just
//! to make a voltage step. It solves the AC active power equations for the
//! bus angles at the planning dispatch and reads off the reactive power each
//! bus must receive to hold its voltage: its load's
//! reactive demand plus what flows out of it into its branches, which counts
//! series absorption, line charging and the reactive share resistance moves
//! toward the receiving end. A generator bus covers that from
//! [`GENERATOR_Q_PLANNING_SHARE`] of its machine capability; whatever is left
//! above [`MIN_SHUNT_BANK_MVAR`] becomes a capacitor bank (a deficit) or a
//! shunt reactor (a surplus). At the planning dispatch the AC power flow then
//! has the planned profile as a solution; the validation stage checks it at
//! the dispatch the DC OPF actually chooses.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::electrical::branches::{DraftBranch, BASE_MVA};
use crate::electrical::generators::DraftGenerator;
use crate::electrical::params::{
    GENERATOR_Q_PLANNING_SHARE, GENERATOR_VG, MIN_SHUNT_BANK_MVAR, VOLTAGE_SUPPORT_KM,
    VOLTAGE_SUPPORT_Q_SHARE,
};
use crate::electrical::planning::dc_angles;
use crate::report::BusKind;
use crate::request::Fuel;
use crate::site::DraftBus;
use crate::topology::Corridor;

/// Fast decoupled iterations before the angle solve gives up.
const MAX_ANGLE_ITERATIONS: usize = 40;
/// Active power mismatch the angle solve accepts, per unit.
const ANGLE_TOLERANCE_PU: f64 = 1e-7;

/// A fixed shunt bank: positive MVAr (at 1 pu) is a capacitor, negative a
/// reactor.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftShunt {
    pub bus: usize,
    pub mvar: f64,
}

impl DraftShunt {
    pub fn kind(&self) -> &'static str {
        if self.mvar >= 0.0 {
            "capacitor"
        } else {
            "reactor"
        }
    }
}

/// The reactive plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactivePlan {
    pub shunts: Vec<DraftShunt>,
    /// True when the AC angle solve met its tolerance at the planned profile.
    pub angles_converged: bool,
    /// Largest angle difference across one branch at the planned profile,
    /// degrees.
    pub max_angle_difference_deg: f64,
}

/// Active and reactive power leaving `from` and `to` into one branch,
/// per unit, for the π model with series `r + jx`, total charging `b` and a
/// nominal ratio: `(p_from, q_from, p_to, q_to)`.
fn branch_flow(br: &DraftBranch, vf: f64, vt: f64, theta: f64) -> (f64, f64, f64, f64) {
    let d = br.r_pu * br.r_pu + br.x_pu * br.x_pu;
    let (g, bs) = (br.r_pu / d, -br.x_pu / d);
    let half = 0.5 * br.b_pu;
    let (s, c) = theta.sin_cos();
    let pf = vf * vf * g - vf * vt * (g * c + bs * s);
    let qf = -vf * vf * (bs + half) - vf * vt * (g * s - bs * c);
    let pt = vt * vt * g - vf * vt * (g * c - bs * s);
    let qt = -vt * vt * (bs + half) + vf * vt * (g * s + bs * c);
    (pf, qf, pt, qt)
}

/// Bus angles of the AC power flow with every voltage magnitude held at `vm`,
/// by fast decoupled P-θ iterations (Stott and Alsaç, "Fast decoupled load
/// flow", IEEE Trans. PAS-93, 1974): `Δθ = B'⁻¹ ΔP / |V|` with `B'` the series
/// reactance Laplacian. `injection` is per unit; each `grounds` bus is its
/// component's slack. Returns the angles and whether they converged.
pub fn ac_angles(
    n: usize,
    branches: &[DraftBranch],
    vm: &[f64],
    injection: &[f64],
    grounds: &[usize],
) -> (Vec<f64>, bool) {
    let edges: Vec<(usize, usize, f64)> = branches
        .iter()
        .map(|b| (b.from, b.to, 1.0 / b.x_pu))
        .collect();
    let mut theta = dc_angles(n, &edges, injection, grounds);
    let mut slack = vec![false; n];
    for &g in grounds {
        slack[g] = true;
    }
    for _ in 0..MAX_ANGLE_ITERATIONS {
        let mut p = vec![0.0; n];
        for br in branches {
            let (pf, _, pt, _) =
                branch_flow(br, vm[br.from], vm[br.to], theta[br.from] - theta[br.to]);
            p[br.from] += pf;
            p[br.to] += pt;
        }
        let mismatch: Vec<f64> = (0..n)
            .map(|i| {
                if slack[i] {
                    0.0
                } else {
                    (injection[i] - p[i]) / vm[i]
                }
            })
            .collect();
        let worst = mismatch.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        if worst < ANGLE_TOLERANCE_PU {
            return (theta, true);
        }
        if !worst.is_finite() {
            break;
        }
        let step = dc_angles(n, &edges, &mismatch, grounds);
        for i in 0..n {
            theta[i] += step[i];
        }
    }
    (theta, false)
}

/// Corridor-km from every bus to its nearest bus in `sources` (multi-source
/// Dijkstra), and that source; `f64::INFINITY` for a bus no source reaches.
fn distance_to_sources(
    n: usize,
    corridors: &[Corridor],
    sources: &[usize],
) -> (Vec<f64>, Vec<usize>) {
    #[derive(PartialEq)]
    struct Item(f64, usize, usize);
    impl Eq for Item {}
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            other
                .0
                .total_cmp(&self.0)
                .then_with(|| other.1.cmp(&self.1))
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for c in corridors {
        adj[c.a].push((c.b, c.length_km));
        adj[c.b].push((c.a, c.length_km));
    }
    let mut dist = vec![f64::INFINITY; n];
    let mut from = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    for &s in sources {
        if dist[s] > 0.0 {
            dist[s] = 0.0;
            from[s] = s;
            heap.push(Item(0.0, s, s));
        }
    }
    while let Some(Item(d, i, src)) = heap.pop() {
        if d > dist[i] {
            continue;
        }
        for &(j, km) in &adj[i] {
            let nd = d + km;
            if nd < dist[j] {
                dist[j] = nd;
                from[j] = src;
                heap.push(Item(nd, j, src));
            }
        }
    }
    (dist, from)
}

/// Synchronous condensers for remote load pockets: while some loaded bus lies
/// more than [`VOLTAGE_SUPPORT_KM`] of corridor from the nearest generator or
/// condenser, the farthest one gets a condenser. Each is sized to
/// [`VOLTAGE_SUPPORT_Q_SHARE`] of the load it then lies nearest to. A pocket
/// fed over long lines with no machine of its own has nothing to hold its
/// voltage, and its AC power flow can stall with the lines well inside their
/// ratings; the condenser gives it the local voltage control real remote
/// systems have. Returns the condensers, to append to the generators.
pub fn place_voltage_support(
    buses: &mut [DraftBus],
    corridors: &[Corridor],
    generators: &[DraftGenerator],
) -> Vec<DraftGenerator> {
    let n = buses.len();
    let mut sources: Vec<usize> = generators.iter().map(|g| g.bus).collect();
    let mut placed: Vec<usize> = Vec::new();
    loop {
        let (dist, _) = distance_to_sources(n, corridors, &sources);
        let farthest = (0..n)
            .filter(|&i| buses[i].load_mw > 0.0 && dist[i].is_finite())
            .max_by(|&a, &b| dist[a].total_cmp(&dist[b]).then(b.cmp(&a)));
        match farthest {
            Some(i) if dist[i] > VOLTAGE_SUPPORT_KM => {
                sources.push(i);
                placed.push(i);
            }
            _ => break,
        }
    }
    if placed.is_empty() {
        return Vec::new();
    }
    let (_, from) = distance_to_sources(n, corridors, &sources);
    let mut served = vec![0.0; n];
    for i in 0..n {
        if from[i] != usize::MAX {
            served[from[i]] += buses[i].load_mw;
        }
    }
    placed
        .into_iter()
        .map(|bus| {
            let q = (VOLTAGE_SUPPORT_Q_SHARE * served[bus]).max(MIN_SHUNT_BANK_MVAR);
            if buses[bus].kind != BusKind::Generator {
                buses[bus].kind = BusKind::Generator;
                buses[bus].annotation = format!(
                    "{} (synchronous condenser: more than {VOLTAGE_SUPPORT_KM:.0} km of corridor from any generator)",
                    buses[bus].annotation
                );
            }
            DraftGenerator {
                bus,
                fuel: Fuel::Other,
                pmax_mw: 0.0,
                pmin_mw: 0.0,
                qmax_mvar: q,
                qmin_mvar: -q,
                c2: 0.0,
                c1: 0.0,
                c0: 0.0,
                makeup: false,
                slack: false,
                condenser: true,
            }
        })
        .collect()
}

/// Plan the shunt banks that hold the planned voltage profile at the planning
/// dispatch. `dispatch_mw` is per generator; `grounds` holds each electrical
/// component's slack bus.
pub fn plan_shunts(
    buses: &[DraftBus],
    branches: &[DraftBranch],
    generators: &[DraftGenerator],
    dispatch_mw: &[f64],
    grounds: &[usize],
) -> ReactivePlan {
    let n = buses.len();
    let vm = vec![GENERATOR_VG; n];
    let mut qmax = vec![0.0; n];
    let mut qmin = vec![0.0; n];
    let mut injection: Vec<f64> = buses.iter().map(|b| -b.load_mw / BASE_MVA).collect();
    for (g, pg) in generators.iter().zip(dispatch_mw) {
        qmax[g.bus] += GENERATOR_Q_PLANNING_SHARE * g.qmax_mvar;
        qmin[g.bus] += GENERATOR_Q_PLANNING_SHARE * g.qmin_mvar;
        injection[g.bus] += pg / BASE_MVA;
    }
    let (theta, angles_converged) = ac_angles(n, branches, &vm, &injection, grounds);
    let mut need: Vec<f64> = buses.iter().map(|b| b.load_mvar).collect();
    let mut max_angle: f64 = 0.0;
    for br in branches {
        let delta = theta[br.from] - theta[br.to];
        max_angle = max_angle.max(delta.abs());
        let (_, qf, _, qt) = branch_flow(br, vm[br.from], vm[br.to], delta);
        need[br.from] += qf * BASE_MVA;
        need[br.to] += qt * BASE_MVA;
    }
    let shunts = need
        .iter()
        .enumerate()
        .filter_map(|(i, &q)| {
            // The bank's rating is stated at 1 pu; at the planned voltage it
            // supplies `rating × V²`.
            let bank = (q - q.clamp(qmin[i], qmax[i])) / (vm[i] * vm[i]);
            (bank.abs() >= MIN_SHUNT_BANK_MVAR).then(|| DraftShunt {
                bus: i,
                mvar: (bank * 10.0).round() / 10.0,
            })
        })
        .collect();
    ReactivePlan {
        shunts,
        angles_converged,
        max_angle_difference_deg: max_angle.to_degrees(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::ElementSource;
    use crate::topology::CorridorRole;

    #[test]
    fn a_load_beyond_the_reach_of_any_generator_gets_a_condenser() {
        // A radial chain of 200 km corridors: a generator at bus 0, 100 MW at
        // every other bus. Buses 1 and 2 lie within 480 km of the machine;
        // bus 3 (600 km) and bus 4 (800 km) do not, and one condenser at the
        // far end brings both within reach.
        let bus = |load_mw: f64| DraftBus {
            lon: 0.0,
            lat: 0.0,
            name: String::new(),
            place: None,
            tier: 0,
            base_kv: 230.0,
            kind: BusKind::Load,
            confidence: 1.0,
            polygon: 0,
            island: 0,
            source: ElementSource::Synthetic,
            population_weight: 0.0,
            load_mw,
            load_mvar: 0.0,
            annotation: String::new(),
        };
        let mut buses: Vec<DraftBus> = (0..5)
            .map(|i| bus(if i == 0 { 0.0 } else { 100.0 }))
            .collect();
        let corridors: Vec<Corridor> = (0..4)
            .map(|i| Corridor {
                a: i,
                b: i + 1,
                kv: 230.0,
                length_km: 200.0,
                role: CorridorRole::Internal,
                circuits: 1,
                route: Vec::new(),
                is_delaunay: true,
            })
            .collect();
        let generator = DraftGenerator {
            bus: 0,
            fuel: Fuel::Gas,
            pmax_mw: 500.0,
            pmin_mw: 0.0,
            qmax_mvar: 275.0,
            qmin_mvar: -225.0,
            c2: 0.01,
            c1: 20.0,
            c0: 0.0,
            makeup: false,
            slack: true,
            condenser: false,
        };
        let condensers =
            place_voltage_support(&mut buses, &corridors, std::slice::from_ref(&generator));
        assert_eq!(condensers.len(), 1);
        let c = &condensers[0];
        assert_eq!(c.bus, 4);
        assert!(c.condenser && c.pmax_mw == 0.0 && c.pmin_mw == 0.0);
        // It lies nearest to buses 3 and 4 (and the tie at 400 km goes to the
        // generator): 200 MW served, so a ±100 MVAr range.
        assert!((c.qmax_mvar - 100.0).abs() < 1e-9 && (c.qmin_mvar + 100.0).abs() < 1e-9);
        assert_eq!(buses[4].kind, BusKind::Generator);
        // Within reach, nothing is added.
        let short = &corridors[..2];
        let mut near: Vec<DraftBus> = buses[..3].to_vec();
        assert!(place_voltage_support(&mut near, short, &[generator]).is_empty());
    }

    fn line(from: usize, to: usize, r: f64, x: f64, b: f64) -> DraftBranch {
        DraftBranch {
            corridor: 0,
            circuits: 1,
            circuit_index: 1,
            from,
            to,
            kv: 230.0,
            class_kv: 230.0,
            r_pu: r,
            x_pu: x,
            b_pu: b,
            rate_a: 500.0,
            is_transformer: false,
            transformer_mva: None,
            winding_ratio: None,
            length_km: 100.0,
            rating_basis: "thermal",
            role: CorridorRole::Internal,
            route: Vec::new(),
        }
    }

    #[test]
    fn branch_flow_conserves_power_and_matches_the_lossless_formula() {
        let br = line(0, 1, 0.0, 0.1, 0.0);
        let (pf, qf, pt, qt) = branch_flow(&br, 1.0, 1.0, 0.3);
        assert!((pf - 0.3f64.sin() / 0.1).abs() < 1e-12);
        assert!((pf + pt).abs() < 1e-12);
        // Reactive loss 2(1 - cos θ)/x is shared equally at equal magnitudes.
        let loss = 2.0 * (1.0 - 0.3f64.cos()) / 0.1;
        assert!((qf + qt - loss).abs() < 1e-12 && (qf - qt).abs() < 1e-12);
        // With resistance the sending end supplies less reactive power than
        // the receiving end needs, and the active loss is r·|I|².
        let br = line(0, 1, 0.02, 0.1, 0.0);
        let (pf, qf, pt, qt) = branch_flow(&br, 1.0, 1.0, 0.3);
        assert!(qf < qt);
        let i2 = (2.0 - 2.0 * 0.3f64.cos()) / (0.02f64.powi(2) + 0.01);
        assert!((pf + pt - 0.02 * i2).abs() < 1e-12);
    }

    #[test]
    fn angles_meet_the_active_power_schedule() {
        // Two lines in series from the slack: 1.0 pu delivered to bus 2.
        let branches = [line(0, 1, 0.01, 0.1, 0.02), line(1, 2, 0.01, 0.1, 0.02)];
        let vm = [1.02, 1.0, 1.0];
        let (theta, ok) = ac_angles(3, &branches, &vm, &[0.0, 0.0, -1.0], &[0]);
        assert!(ok);
        let (_, _, pt, _) = branch_flow(&branches[1], vm[1], vm[2], theta[1] - theta[2]);
        assert!((pt + 1.0).abs() < 1e-6, "{pt}");
    }
}
