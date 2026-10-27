//! Branch parameters: per-km class values times the corridor length with a
//! bounded log-normal spread, charging from the class table, a rating that is
//! the lesser of the conductor thermal limit and the St. Clair loadability,
//! and transformer impedance in series where the endpoints differ in voltage.
//! A rating is never raised above what the conductor and the line length
//! allow: a corridor that must carry more gets more circuits from the
//! corridor plan ([`crate::electrical::planning`]). Every circuit of one
//! corridor is built to the same design, as the circuits of a double-circuit
//! tower line are.

use crate::electrical::generators::DraftGenerator;
use crate::electrical::params::{
    class_ratio, line_class_for, st_clair_loadability, transformer_class_for, SPREAD_SIGMA_HIGH,
    SPREAD_SIGMA_LOW, TRANSFORMER_HEADROOM, TRANSFORMER_MIN_MVA,
};
use crate::request::Fidelity;
use crate::rng::Stream;
use crate::site::DraftBus;
use crate::topology::{Corridor, CorridorRole};

/// The electrical design of one circuit of a corridor.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitDesign {
    /// Higher voltage end.
    pub from: usize,
    pub to: usize,
    pub kv: f64,
    pub class_kv: f64,
    pub r_pu: f64,
    pub x_pu: f64,
    pub b_pu: f64,
    pub rate_a: f64,
    pub is_transformer: bool,
    pub transformer_mva: Option<f64>,
    pub winding_ratio: Option<f64>,
    pub length_km: f64,
    pub rating_basis: &'static str,
    /// The line alone, before any transformer: series r and x (pu), rating
    /// (MVA, before the spread) and its basis.
    pub line_r_pu: f64,
    pub line_x_pu: f64,
    pub line_rate: f64,
    pub line_basis: &'static str,
    /// The circuit's rating spread factor.
    pub rate_spread: f64,
}

impl CircuitDesign {
    /// Rate the circuit's step-down transformer at `mva`: its impedance
    /// follows the size class, and the circuit rating becomes the lesser of
    /// the transformer and the line.
    pub fn set_transformer_mva(&mut self, mva: f64) {
        let (rt, xt) = transformer_impedance(mva);
        self.r_pu = self.line_r_pu + rt;
        self.x_pu = self.line_x_pu + xt;
        let (rate, basis) = if mva < self.line_rate {
            (mva, "transformer")
        } else {
            (self.line_rate, self.line_basis)
        };
        self.rate_a = rate * self.rate_spread;
        self.rating_basis = basis;
        self.transformer_mva = Some(mva);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftBranch {
    pub corridor: usize,
    /// Parallel circuits in this branch's corridor, this one included.
    pub circuits: u32,
    /// 1-based position of this branch among them.
    pub circuit_index: u32,
    pub from: usize,
    pub to: usize,
    pub kv: f64,
    pub class_kv: f64,
    pub r_pu: f64,
    pub x_pu: f64,
    pub b_pu: f64,
    pub rate_a: f64,
    pub is_transformer: bool,
    pub transformer_mva: Option<f64>,
    pub winding_ratio: Option<f64>,
    pub length_km: f64,
    pub rating_basis: &'static str,
    pub role: CorridorRole,
    pub route: Vec<(f64, f64)>,
}

/// System base, MVA.
pub const BASE_MVA: f64 = 100.0;

/// Midpoint offset between neighbouring parallel circuits on the map, degrees.
const PARALLEL_ROUTE_OFFSET_DEG: f64 = 0.006;

pub fn spread_sigma(fidelity: Fidelity) -> f64 {
    match fidelity {
        Fidelity::High => SPREAD_SIGMA_HIGH,
        Fidelity::Low => SPREAD_SIGMA_LOW,
    }
}

/// Per-unit line parameters on `BASE_MVA` and `kv` for `length_km` of the
/// class nearest `kv`, with impedance spread `spread`: `(r, x, b, rating MVA,
/// rating basis)`. A level between classes (220 kV, 34.5 kV) keeps the
/// class's conductors at its own voltage: the thermal rating scales with the
/// voltage at the same ampacity (`S = √3 V I`) and the surge impedance
/// loading with its square at the same surge impedance (`SIL = V² / Zc`).
pub fn line_parameters(kv: f64, length_km: f64, spread: f64) -> (f64, f64, f64, f64, &'static str) {
    let (class, ratio) = class_ratio(kv);
    let z_base = kv * kv / BASE_MVA;
    let x_ohm = class.x_ohm_per_km * length_km * spread;
    let r_ohm = class.r_ohm_per_km * length_km * spread;
    let b_siemens = class.b_us_per_km * 1e-6 * length_km;
    let thermal = class.thermal_mva * ratio;
    let loadability = class.sil_mw * ratio * ratio * st_clair_loadability(length_km);
    let (rate, basis) = if loadability < thermal {
        (loadability, "loadability")
    } else {
        (thermal, "thermal")
    };
    (
        r_ohm / z_base,
        x_ohm / z_base,
        b_siemens * z_base,
        rate,
        basis,
    )
}

/// Per-unit transformer impedance on `BASE_MVA` for a unit of `mva`.
pub fn transformer_impedance(mva: f64) -> (f64, f64) {
    let class = transformer_class_for(mva);
    let z = class.z_pu_own_base * BASE_MVA / mva;
    let x = z / (1.0 + 1.0 / (class.x_over_r * class.x_over_r)).sqrt();
    (x / class.x_over_r, x)
}

/// The random part of a corridor's design, drawn once so that redesigning a
/// corridor at another voltage (the corridor plan may promote its ends)
/// draws nothing new.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorridorDraw {
    /// Bounded log-normal impedance spread.
    pub spread: f64,
    /// Rating spread below the class figure.
    pub rate_spread: f64,
}

/// One draw per corridor, in corridor order.
pub fn draw_corridors(
    corridors: &[Corridor],
    fidelity: Fidelity,
    rng: &mut Stream,
) -> Vec<CorridorDraw> {
    let sigma = spread_sigma(fidelity);
    corridors
        .iter()
        .map(|_| CorridorDraw {
            spread: (sigma * rng.normal().clamp(-3.0, 3.0)).exp(),
            // A mild rating spread below the class figure.
            rate_spread: 0.95 + 0.05 * rng.uniform(),
        })
        .collect()
}

/// One circuit design per corridor at the buses' current voltages. A
/// corridor whose ends differ in voltage carries its line at the higher
/// voltage and a step-down transformer at the lower end, sized for that
/// bus's load or generation (whichever is larger) with planning headroom,
/// shared over the bus's transformer corridors.
pub fn design_circuits(
    buses: &[DraftBus],
    corridors: &[Corridor],
    generators: &[DraftGenerator],
    draws: &[CorridorDraw],
) -> Vec<CircuitDesign> {
    let n = buses.len();
    let mut gen_at = vec![0.0; n];
    for g in generators {
        gen_at[g.bus] += g.pmax_mw;
    }
    let steps_down = |c: &Corridor| (buses[c.a].base_kv - buses[c.b].base_kv).abs() > 1e-6;
    let mut transformer_corridors = vec![0usize; n];
    for c in corridors.iter().filter(|c| steps_down(c)) {
        let lo = if buses[c.a].base_kv < buses[c.b].base_kv {
            c.a
        } else {
            c.b
        };
        transformer_corridors[lo] += 1;
    }
    corridors
        .iter()
        .zip(draws)
        .map(|(c, draw)| {
            let (hi, lo) = if buses[c.a].base_kv >= buses[c.b].base_kv {
                (c.a, c.b)
            } else {
                (c.b, c.a)
            };
            let kv = buses[hi].base_kv;
            let transformer = steps_down(c);
            let (r, x, b, rate, basis) = line_parameters(kv, c.length_km, draw.spread);
            let mut design = CircuitDesign {
                from: hi,
                to: lo,
                kv,
                class_kv: line_class_for(kv).kv,
                r_pu: r,
                x_pu: x,
                b_pu: b,
                rate_a: rate * draw.rate_spread,
                is_transformer: transformer,
                transformer_mva: None,
                winding_ratio: transformer.then(|| buses[hi].base_kv / buses[lo].base_kv),
                length_km: c.length_km,
                rating_basis: basis,
                line_r_pu: r,
                line_x_pu: x,
                line_rate: rate,
                line_basis: basis,
                rate_spread: draw.rate_spread,
            };
            if transformer {
                let served = buses[lo].load_mw.max(gen_at[lo]) * TRANSFORMER_HEADROOM
                    / transformer_corridors[lo].max(1) as f64;
                design.set_transformer_mva(served.max(TRANSFORMER_MIN_MVA));
            }
            design
        })
        .collect()
}

/// Expand every corridor into `corridor.circuits` branches of its design.
/// Parallel circuits are drawn with their midpoints offset to alternating
/// sides so they read as separate lines on the map.
pub fn expand_circuits(corridors: &[Corridor], designs: &[CircuitDesign]) -> Vec<DraftBranch> {
    let mut out = Vec::new();
    for (ci, (c, d)) in corridors.iter().zip(designs).enumerate() {
        for circuit in 1..=c.circuits {
            let mut route = c.route.clone();
            if circuit > 1 && route.len() == 3 {
                let (dx, dy) = (route[2].0 - route[0].0, route[2].1 - route[0].1);
                let len = dx.hypot(dy).max(1e-12);
                let side = if circuit % 2 == 0 { 1.0 } else { -1.0 };
                let step = PARALLEL_ROUTE_OFFSET_DEG * (circuit / 2) as f64 * side;
                route[1].0 += -dy / len * step;
                route[1].1 += dx / len * step;
            }
            if d.from != c.a {
                route.reverse();
            }
            out.push(DraftBranch {
                corridor: ci,
                circuits: c.circuits,
                circuit_index: circuit,
                from: d.from,
                to: d.to,
                kv: d.kv,
                class_kv: d.class_kv,
                r_pu: d.r_pu,
                x_pu: d.x_pu,
                b_pu: d.b_pu,
                rate_a: d.rate_a,
                is_transformer: d.is_transformer,
                transformer_mva: d.transformer_mva,
                winding_ratio: d.winding_ratio,
                length_km: d.length_km,
                rating_basis: d.rating_basis,
                role: c.role,
                route,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_loadability_limited_and_short_ones_thermal() {
        let (r, x, b, rate, basis) = line_parameters(500.0, 40.0, 1.0);
        assert!(r > 0.0 && x > r && b > 0.0);
        assert_eq!(rate, 2500.0);
        assert_eq!(basis, "thermal");
        let (_, _, _, rate, basis) = line_parameters(500.0, 400.0, 1.0);
        assert!(rate < 2500.0);
        assert_eq!(basis, "loadability");
        // 230 kV, 100 km: x ≈ 0.45 Ω/km × 100 / (230² / 100) ≈ 0.085 pu.
        let (_, x, _, _, _) = line_parameters(230.0, 100.0, 1.0);
        assert!((x - 0.0851).abs() < 0.002, "{x}");
    }

    #[test]
    fn loadability_keeps_the_line_within_the_stability_margin() {
        // At its St. Clair rating a line's DC angle is loadability × electrical
        // length, inside the 44.4° steady-state margin for every class, any
        // level between classes, and any length.
        let margin = crate::electrical::params::MAX_ANGLE_SPREAD_RAD;
        for kv in [
            34.5, 69.0, 115.0, 138.0, 161.0, 220.0, 230.0, 330.0, 345.0, 400.0, 500.0, 765.0,
        ] {
            for length in [20.0, 80.0, 160.0, 320.0, 480.0, 800.0] {
                let (_, x, _, rate, _) = line_parameters(kv, length, 1.0);
                let angle = rate / BASE_MVA * x;
                assert!(
                    angle < margin,
                    "{kv} kV {length} km: {:.1}°",
                    angle.to_degrees()
                );
            }
        }
    }

    #[test]
    fn levels_between_classes_scale_ratings_with_voltage() {
        // 34.5 kV on 138 kV conductors: a quarter of the thermal MVA and a
        // sixteenth of the SIL.
        let (_, _, _, rate, basis) = line_parameters(34.5, 10.0, 1.0);
        assert_eq!(basis, "loadability");
        assert!((rate - 50.0 / 16.0 * 3.0).abs() < 1e-9, "{rate}");
        let r: f64 = 161.0 / 138.0;
        let (_, _, _, rate, _) = line_parameters(161.0, 10.0, 1.0);
        assert!(
            (rate - (180.0 * r).min(50.0 * r * r * 3.0)).abs() < 1e-9,
            "{rate}"
        );
    }

    #[test]
    fn transformer_impedance_follows_class_and_base() {
        let (r, x) = transformer_impedance(100.0);
        assert!((x - 0.08).abs() < 0.001);
        assert!((x / r - 25.0).abs() < 1e-9);
        let (_, x) = transformer_impedance(500.0);
        assert!((x - 0.12 * 100.0 / 500.0).abs() < 0.001);
    }
}
