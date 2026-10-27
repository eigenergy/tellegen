//! Physical parameter tables. Every constant carries its source. Per unit
//! values are on a 100 MVA system base and the branch's nominal kV.

use crate::request::Fuel;

/// A transmission line class: representative ACSR bundles per nominal voltage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineClass {
    /// Nominal class voltage, kV.
    pub kv: f64,
    /// Positive-sequence series resistance, Ω/km.
    pub r_ohm_per_km: f64,
    /// Positive-sequence series reactance, Ω/km.
    pub x_ohm_per_km: f64,
    /// Positive-sequence shunt (charging) susceptance, µS/km.
    pub b_us_per_km: f64,
    /// Conductor thermal rating, MVA.
    pub thermal_mva: f64,
    /// Surge impedance loading, MW.
    pub sil_mw: f64,
}

/// Line classes.
///
/// source: r, x, b per km from Glover, Overbye, Sarma, *Power System Analysis
/// and Design* (6th ed.), Table A.4 and Example 4.x bundle geometries
/// (115/138 kV single 795 kcmil ACSR; 230 kV single 1272 kcmil; 345 kV 2×954
/// kcmil; 500 kV 3×954 kcmil; 765 kV 4×954 kcmil); thermal ratings from the
/// same conductor ampacities; SIL from Kundur, *Power System Stability and
/// Control*, Table 6.1 (115 kV ≈ 50 MW, 230 kV ≈ 140 MW, 345 kV ≈ 400 MW,
/// 500 kV ≈ 900 MW, 765 kV ≈ 2200 MW); the class set matches Birchfield et
/// al. 2017, Table II.
pub const LINE_CLASSES: [LineClass; 5] = [
    LineClass {
        kv: 138.0,
        r_ohm_per_km: 0.11,
        x_ohm_per_km: 0.48,
        b_us_per_km: 3.4,
        thermal_mva: 180.0,
        sil_mw: 50.0,
    },
    LineClass {
        kv: 230.0,
        r_ohm_per_km: 0.06,
        x_ohm_per_km: 0.45,
        b_us_per_km: 3.8,
        thermal_mva: 500.0,
        sil_mw: 140.0,
    },
    LineClass {
        kv: 345.0,
        r_ohm_per_km: 0.03,
        x_ohm_per_km: 0.35,
        b_us_per_km: 4.8,
        thermal_mva: 1200.0,
        sil_mw: 400.0,
    },
    LineClass {
        kv: 500.0,
        r_ohm_per_km: 0.02,
        x_ohm_per_km: 0.30,
        b_us_per_km: 5.6,
        thermal_mva: 2500.0,
        sil_mw: 900.0,
    },
    LineClass {
        kv: 765.0,
        r_ohm_per_km: 0.012,
        x_ohm_per_km: 0.26,
        b_us_per_km: 6.5,
        thermal_mva: 4500.0,
        sil_mw: 2200.0,
    },
];

/// The class whose nominal voltage is nearest `kv` (log distance, so 161 kV
/// snaps to 138 and 220 kV to 230). The class supplies per-km parameters; the
/// branch keeps its own kV for the per-unit base.
pub fn line_class_for(kv: f64) -> &'static LineClass {
    let target = kv.max(1.0).ln();
    LINE_CLASSES
        .iter()
        .min_by(|a, b| {
            (a.kv.ln() - target)
                .abs()
                .partial_cmp(&(b.kv.ln() - target).abs())
                .unwrap()
        })
        .expect("table is non-empty")
}

/// Index of a class in [`LINE_CLASSES`].
pub fn line_class_index(class: &LineClass) -> usize {
    LINE_CLASSES
        .iter()
        .position(|c| c.kv == class.kv)
        .expect("class from the table")
}

/// St. Clair loadability in multiples of SIL as a function of line length.
///
/// source: Dunlop, Gutman, Marchenko, "Analytical development of loadability
/// characteristics for EHV and UHV transmission lines", IEEE Trans. PAS-98
/// (1979), Fig. 2: about 3.0 SIL at 50 mi (80 km), 2.0 at 100 mi (160 km),
/// 1.35 at 200 mi (320 km), 1.0 at 300 mi (480 km), 0.85 at 400 mi (640 km),
/// 0.6 at 600 mi (965 km). Log-linear between the knots, flat below 80 km,
/// extrapolated as `0.6 · (965 / L)^0.5` beyond, floored at 0.4 SIL.
pub fn st_clair_loadability(length_km: f64) -> f64 {
    const KNOTS: [(f64, f64); 6] = [
        (80.0, 3.0),
        (160.0, 2.0),
        (320.0, 1.35),
        (480.0, 1.0),
        (640.0, 0.85),
        (965.0, 0.6),
    ];
    if length_km <= KNOTS[0].0 {
        return KNOTS[0].1;
    }
    for w in KNOTS.windows(2) {
        let ((l0, m0), (l1, m1)) = (w[0], w[1]);
        if length_km <= l1 {
            let t = (length_km.ln() - l0.ln()) / (l1.ln() - l0.ln());
            return m0 + (m1 - m0) * t;
        }
    }
    (0.6 * (965.0 / length_km).sqrt()).max(0.4)
}

/// A two-winding transformer class by size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformerClass {
    /// Upper MVA bound of the class (inclusive); the last class is open.
    pub max_mva: f64,
    /// Impedance on the transformer's own base, per unit.
    pub z_pu_own_base: f64,
    pub x_over_r: f64,
}

/// Transformer classes.
///
/// source: IEEE C57.12.10 Table 10 impedance ranges for power transformers
/// (7.5–10.5% up to 100 MVA, 9–12% above) and Grainger & Stevenson, *Power
/// System Analysis*, Table A.1 X/R ratios rising with size (about 25 at
/// 50 MVA, 35 at 200 MVA, 45 above 400 MVA).
pub const TRANSFORMER_CLASSES: [TransformerClass; 3] = [
    TransformerClass {
        max_mva: 100.0,
        z_pu_own_base: 0.08,
        x_over_r: 25.0,
    },
    TransformerClass {
        max_mva: 400.0,
        z_pu_own_base: 0.10,
        x_over_r: 35.0,
    },
    TransformerClass {
        max_mva: f64::INFINITY,
        z_pu_own_base: 0.12,
        x_over_r: 45.0,
    },
];

/// Smallest transformer the generator places, MVA.
pub const TRANSFORMER_MIN_MVA: f64 = 100.0;

/// Step to which the corridor plan rounds an enlarged transformer bank, MVA.
/// source: preferred power transformer ratings advance in steps of tens of
/// MVA (IEEE C57.12.10); 50 MVA is a coarse but standard granularity.
pub const TRANSFORMER_SIZE_STEP_MVA: f64 = 50.0;

/// Transformer size factor over the larger of local load and connected
/// generation (firm capacity with headroom). source: common utility planning
/// criterion of 20% headroom on substation transformation.
pub const TRANSFORMER_HEADROOM: f64 = 1.2;

pub fn transformer_class_for(mva: f64) -> &'static TransformerClass {
    TRANSFORMER_CLASSES
        .iter()
        .find(|c| mva <= c.max_mva)
        .unwrap_or(&TRANSFORMER_CLASSES[2])
}

/// Emergency rating multipliers. source: NERC FAC-008 practice; utilities
/// commonly state short-term emergency ratings 10% and 20% above normal.
pub const RATE_B_FACTOR: f64 = 1.1;
pub const RATE_C_FACTOR: f64 = 1.2;

/// Bounded log-normal spread on per-branch reactance (σ of ln x) at high and
/// low fidelity. source: the spread of per-km reactance across real 230 kV
/// line designs in Glover et al. Table A.4 is about ±10%.
pub const SPREAD_SIGMA_HIGH: f64 = 0.08;
pub const SPREAD_SIGMA_LOW: f64 = 0.16;

/// Planning voltage band for generated buses, per unit. source: ANSI C84.1
/// Range A at transmission level and typical planning criteria (±5%).
pub const BUS_VMAX: f64 = 1.05;
pub const BUS_VMIN: f64 = 0.95;

/// Generator voltage set point, per unit. source: Birchfield et al. 2017,
/// Section III-D, generator set points 1.0–1.05 with 1.02 typical.
pub const GENERATOR_VG: f64 = 1.02;

/// Reactive limits as a fraction of `pmax`. source: typical synchronous
/// machine capability (0.85 lagging / 0.95 leading power factor) gives
/// qmax ≈ 0.55 and qmin ≈ −0.45 of rated MW (Kundur, Section 5.4).
pub const QMAX_FRACTION: f64 = 0.55;
pub const QMIN_FRACTION: f64 = -0.45;

/// The class a nominal voltage takes its parameters from, and the ratio of
/// that voltage to the class voltage. The same conductors and banks at the
/// bus's own voltage carry power in proportion to it (`S = √3 V I`), so
/// capacities quoted per class scale by this ratio.
pub fn class_ratio(kv: f64) -> (&'static LineClass, f64) {
    let class = line_class_for(kv);
    (class, kv / class.kv)
}

/// Largest load one bus at `kv` carries before the bus is promoted to the
/// next level, MW: the class figure scaled to the bus's own voltage
/// ([`class_ratio`]). A metro's demand sits on its high-voltage substations,
/// not on a 115 kV bus. source: per-bus load maxima by voltage level in the
/// ACTIVSg2000 case (115/161 kV buses below about 400 MW, 230 kV below about
/// 1,400 MW; Birchfield et al. 2017, Section III-B), and typical substation
/// transformation (two to three banks of the sizes in `TRANSFORMER_CLASSES`).
pub fn bus_load_cap_mw(kv: f64) -> f64 {
    let (class, ratio) = class_ratio(kv);
    class_load_cap_mw(class.kv) * ratio
}

fn class_load_cap_mw(class_kv: f64) -> f64 {
    if class_kv <= 138.0 {
        300.0
    } else if class_kv <= 230.0 {
        1_000.0
    } else if class_kv <= 345.0 {
        2_500.0
    } else if class_kv <= 500.0 {
        7_000.0
    } else {
        f64::INFINITY
    }
}

/// Largest generation one bus at `kv` collects, MW: the class figure scaled
/// to the bus's own voltage ([`class_ratio`]). A plant above the cap of its
/// bus spills onto the next ranked bus, so no single bus exports more than
/// the corridors of its class can plausibly carry.
/// source: EIA-860 (2023) plant inventory and the interconnection voltages of
/// the largest plants: units up to a few hundred MW sit on 115–161 kV
/// (ACTIVSg2000's largest 115/161 kV generator buses stay below about
/// 400 MW), plants near 1 GW on 230 kV, the 2.4–2.7 GW Texas nuclear stations
/// on 345 kV, and the largest U.S. stations on 500 kV (Palo Verde 3.9 GW,
/// Browns Ferry 3.8 GW) or 765 kV (Rockport and Gavin, about 2.6 GW each,
/// with room for more on the stronger system).
pub fn bus_generation_cap_mw(kv: f64) -> f64 {
    let (class, ratio) = class_ratio(kv);
    class_generation_cap_mw(class.kv) * ratio
}

fn class_generation_cap_mw(class_kv: f64) -> f64 {
    if class_kv <= 138.0 {
        400.0
    } else if class_kv <= 230.0 {
        1_200.0
    } else if class_kv <= 345.0 {
        2_700.0
    } else if class_kv <= 500.0 {
        4_000.0
    } else {
        6_000.0
    }
}

/// Load allocation: inverse distance weighting radius and floor. source: the
/// 25 km radius is the distance a sub-transmission network typically serves
/// from one transmission substation. The floor is a quarter of the median
/// place-weighted bus, so rural buses carry a small but non-zero load and a
/// single metro never dictates the countryside's demand.
pub const LOAD_IDW_RADIUS_KM: f64 = 25.0;
pub const LOAD_FLOOR_MEDIAN_FRACTION: f64 = 0.25;

/// Corridor planning: circuits are added to a corridor until the planning
/// dispatch loads none of them above this share of its rating. source:
/// transmission expansion to the flows of the planned dispatch (Garver 1970,
/// "Transmission network estimation using linear programming", IEEE Trans.
/// PAS-89) under the NERC TPL-001 base case requirement that facilities stay
/// within their normal ratings; the 10% margin covers the losses and reactive
/// current a DC flow leaves out.
pub const PLANNING_UTILISATION: f64 = 0.9;

/// Planning rounds (each one DC power flow, adding capacity or stiffening)
/// before the plan is reported as unfinished. Adding circuits pulls flow onto
/// the strengthened corridors and stiffening moves flow again, so the plan
/// iterates; it usually settles within twenty rounds.
pub const PLANNING_MAX_ROUNDS: u32 = 64;

/// Widest angle between two buses of one island in the planning case,
/// radians: asin(0.7) ≈ 44.4°. source: the St. Clair loadability curve keeps
/// a 30% steady-state stability margin (Dunlop, Gutman, Marchenko 1979), so
/// a transfer path runs at most at sin δ = 0.7 of its maximum. A chain of
/// corridors each within its own St. Clair rating can still exceed that end
/// to end, so the plan holds the whole island to it.
pub const MAX_ANGLE_SPREAD_RAD: f64 = 0.775_397_496_610_753;

/// Share of an island's corridors one stiffening round reinforces, largest
/// first-order spread reduction first. A planning heuristic: small enough
/// that each round re-measures the spread before reinforcing more.
pub const STIFFENING_BATCH_SHARE: f64 = 0.05;

/// Branches per bus up to which the plan adds circuits to narrow an island's
/// angle spread; beyond it the spread is reported as unfinished rather than
/// met by a network denser than real ones. source: Birchfield et al. 2017
/// report branch-to-bus ratios of 1.2–1.5 for North American transmission
/// systems (the range of `density_branch_factor`); the budget leaves about a
/// tenth above the densest of them, a modelling choice.
pub const STIFFENING_BRANCH_BUDGET: f64 = 1.6;

/// Share of buses at or above the highest requested level up to which the
/// plan raises buses to narrow an island's angle spread. source: Birchfield
/// et al. 2017, Table II: 8–35% of buses sit at the highest voltage level of
/// the North American systems surveyed (the `hv_share` range of the
/// published calibration targets).
pub const STIFFENING_HV_SHARE: f64 = 0.35;

/// Circuits a corridor may need at its voltage before its ends are raised to
/// the next level. source: transmission planning chooses the voltage class
/// from the power and distance a corridor must carry, through each class's
/// St. Clair loadability (Dunlop, Gutman, Marchenko 1979; EPRI,
/// *Transmission Line Reference Book, 345 kV and Above*): where a corridor
/// would need a third circuit at its voltage, one circuit at the next level
/// carries the same power on less right of way and with a smaller angle.
pub const UPGRADE_CIRCUITS: u32 = 2;

/// Circuits one corridor may hold; a corridor that needs more raises its ends
/// along the voltage ladder (up to the highest level in service, see
/// [`crate::voltage`]) instead. source: double and
/// quadruple circuit lines are common in Germany, India, Japan and China and
/// six-circuit lines rare (Acosta and Tavares, IPST 2019); in the
/// OpenStreetMap-based European grid (Xiong et al., *Scientific Data* 12:277,
/// 2025, Zenodo dataset v0.3) 99.5% of the 7,100 overhead lines at 220 kV and
/// above carry one or two circuits per voltage level and none more than four.
pub const MAX_CIRCUITS_PER_CORRIDOR: u32 = 4;

/// Corridor-km from the nearest generator beyond which a loaded bus gets a
/// synchronous condenser for voltage support. source: the St. Clair curve
/// falls to 1.0 SIL at 480 km (300 mi; Dunlop, Gutman, Marchenko 1979), where
/// voltage-drop and stability limits replace the thermal one, so a load
/// farther than that from any machine depends on remote voltage control alone.
pub const VOLTAGE_SUPPORT_KM: f64 = 480.0;

/// Reactive range of a synchronous condenser, MVAr per MW of the load it lies
/// nearest to: the ±0.33 MVAr/MW of a 0.95 power-factor load, rounded up to
/// leave headroom. Modelling choice.
pub const VOLTAGE_SUPPORT_Q_SHARE: f64 = 0.5;

/// Share of a generator's reactive capability the reactive plan counts on at
/// peak; the rest stays in reserve for contingencies and voltage control.
/// source: WECC, "Voltage Stability Criteria, Undervoltage Load Shedding
/// Strategy, and Reactive Power Reserve Monitoring Methodology" (1998), and
/// NERC TPL-001 voltage stability practice, which keep dynamic reactive
/// reserve on machines rather than scheduling them to their limits in the
/// planning base case.
pub const GENERATOR_Q_PLANNING_SHARE: f64 = 0.5;

/// Smallest shunt bank the reactive plan installs, MVAr; a smaller imbalance
/// is left to the network. source: IEEE Std 1036, *Guide for the Application
/// of Shunt Power Capacitors*: transmission banks switch in steps of a few to
/// a few tens of MVAr, and 5 MVAr is a small 69–138 kV step.
pub const MIN_SHUNT_BANK_MVAR: f64 = 5.0;

/// Unit size distribution (log-normal in MW) and cost ranges by fuel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelParams {
    pub fuel: Fuel,
    /// ln of the median unit size, MW.
    pub size_mu: f64,
    /// σ of ln size.
    pub size_sigma: f64,
    /// Linear cost range, $/MWh.
    pub c1_range: (f64, f64),
    /// Quadratic cost, $/MW²h, kept small and positive so the OPF is strictly convex.
    pub c2: f64,
}

/// Fuel parameters.
///
/// source: unit sizes from the EIA-860 2023 generator inventory (median
/// nameplate: gas combined cycle ≈ 250 MW with wide spread, coal ≈ 400 MW,
/// nuclear ≈ 1000 MW, hydro ≈ 60 MW, wind farms ≈ 150 MW, solar farms ≈
/// 60 MW, battery storage ≈ 50 MW); marginal costs from heat rate × fuel
/// price (EIA Electric Power Monthly 2024: gas ≈ $3.5/MMBtu × 7 MMBtu/MWh,
/// coal ≈ $2.2/MMBtu × 10 MMBtu/MWh, nuclear fuel ≈ $7/MWh plus O&M) and
/// near-zero variable cost for hydro, wind and solar; the ACTIVSg synthetic
/// cases (Xu et al. 2017, "Creation of synthetic electric grid models for
/// transmission system planning") use the same ordering.
pub const FUEL_PARAMS: [FuelParams; 8] = [
    FuelParams {
        fuel: Fuel::Gas,
        size_mu: 5.52,
        size_sigma: 0.7,
        c1_range: (28.0, 42.0),
        c2: 0.004,
    },
    FuelParams {
        fuel: Fuel::Coal,
        size_mu: 5.99,
        size_sigma: 0.5,
        c1_range: (24.0, 36.0),
        c2: 0.003,
    },
    FuelParams {
        fuel: Fuel::Nuclear,
        size_mu: 6.91,
        size_sigma: 0.25,
        c1_range: (8.0, 12.0),
        c2: 0.001,
    },
    FuelParams {
        fuel: Fuel::Hydro,
        size_mu: 4.09,
        size_sigma: 0.8,
        c1_range: (2.0, 5.0),
        c2: 0.002,
    },
    FuelParams {
        fuel: Fuel::Wind,
        size_mu: 5.01,
        size_sigma: 0.6,
        c1_range: (1.0, 3.0),
        c2: 0.002,
    },
    FuelParams {
        fuel: Fuel::Solar,
        size_mu: 4.09,
        size_sigma: 0.7,
        c1_range: (0.5, 2.0),
        c2: 0.002,
    },
    FuelParams {
        fuel: Fuel::Storage,
        size_mu: 3.91,
        size_sigma: 0.6,
        c1_range: (20.0, 30.0),
        c2: 0.004,
    },
    FuelParams {
        fuel: Fuel::Other,
        size_mu: 4.6,
        size_sigma: 0.7,
        c1_range: (30.0, 50.0),
        c2: 0.004,
    },
];

pub fn fuel_params(fuel: Fuel) -> &'static FuelParams {
    FUEL_PARAMS
        .iter()
        .find(|p| p.fuel == fuel)
        .expect("every fuel has parameters")
}

/// Smallest generating unit, MW.
pub const MIN_UNIT_MW: f64 = 5.0;

/// Default share of buses carrying a generator. source: ACTIVSg2000 has 544
/// generators on 2000 buses and Texas7k 731 on 6717; 16% sits between.
pub const GENERATOR_BUS_SHARE: f64 = 0.16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_snap_by_log_distance() {
        assert_eq!(line_class_for(115.0).kv, 138.0);
        assert_eq!(line_class_for(161.0).kv, 138.0);
        assert_eq!(line_class_for(220.0).kv, 230.0);
        assert_eq!(line_class_for(400.0).kv, 345.0);
        assert_eq!(line_class_for(34.5).kv, 138.0);
        assert_eq!(line_class_for(1000.0).kv, 765.0);
    }

    #[test]
    fn st_clair_is_monotone_and_hits_the_knots() {
        assert_eq!(st_clair_loadability(10.0), 3.0);
        assert!((st_clair_loadability(160.0) - 2.0).abs() < 1e-9);
        assert!((st_clair_loadability(480.0) - 1.0).abs() < 1e-9);
        let mut last = f64::INFINITY;
        for l in (10..2000).step_by(10) {
            let m = st_clair_loadability(l as f64);
            assert!(m <= last + 1e-12);
            assert!(m >= 0.4);
            last = m;
        }
    }

    #[test]
    fn transformer_classes_cover_all_sizes() {
        assert_eq!(transformer_class_for(50.0).z_pu_own_base, 0.08);
        assert_eq!(transformer_class_for(250.0).z_pu_own_base, 0.10);
        assert_eq!(transformer_class_for(5000.0).z_pu_own_base, 0.12);
        for f in Fuel::ALL {
            let p = fuel_params(f);
            assert!(p.c2 > 0.0 && p.c1_range.0 < p.c1_range.1);
        }
    }
}
