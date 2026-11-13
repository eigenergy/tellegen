//! Generating units from published data: machine, exciter, governor and
//! stabilizer parameter sets, each with its source, so a unit "like this
//! standard one" can be added and simulated without looking for realistic
//! values first.
//!
//! Machine data are on the unit's own base (per unit of its rating, H in
//! seconds on its rating); a unit of another size takes the same per-unit
//! values, as is usual practice. Values given on the system base in the
//! source (the WSCC 9-bus machines on 100 MVA) are converted. Each set says
//! which values come from the source and which are typical values filled in
//! where the source gives none (subtransient data, saturation, controls).
//!
//! Parameter order is PSS/E's: GENROU T'do T''do T'qo T''qo H D Xd Xq X'd
//! X'q X''d Xl S(1.0) S(1.2); GENSAL T'do T''do T''qo H D Xd Xq X'd X''d Xl
//! S(1.0) S(1.2); SEXS TA/TB TB K TE EMIN EMAX; TGOV1 R T1 VMAX VMIN T2 T3
//! Dt; HYGOV R r Tr Tf Tg VELM GMAX GMIN TW At Dturb qNL; GAST R T1 T2 T3
//! AT KT VMAX VMIN Dturb; IEEEST ICS IB A1..A6 T1..T6 KS LSMAX LSMIN VCU VCL.

use serde::Serialize;

/// One model of a preset: PSS/E name and parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PresetModel {
    pub model: &'static str,
    pub params: &'static str,
}

/// A generating unit from published data.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct UnitPreset {
    pub id: &'static str,
    /// `steam`, `hydro`, `gas` or `inverter`.
    pub kind: &'static str,
    pub label: &'static str,
    /// The unit's rating in the source (MVA); informational, the per-unit
    /// values apply to any rating.
    pub rated_mva: Option<f64>,
    /// The published source.
    pub source: &'static str,
    /// Which values come from the source, which are typical fills.
    pub notes: &'static str,
    pub machine: PresetModel,
    pub exciter: Option<PresetModel>,
    pub governor: Option<PresetModel>,
    pub stabilizer: Option<PresetModel>,
}

const fn m(model: &'static str, params: &'static str) -> PresetModel {
    PresetModel { model, params }
}

/// TGOV1 values commonly used as typical steam governor data (5 % droop,
/// reheat time constant 7 s); not from a specific unit.
const TGOV1_TYPICAL: PresetModel = m("TGOV1", "0.05 0.5 1.0 0.0 2.1 7.0 0.0");
/// SEXS as commonly used typical data for a static exciter.
const SEXS_TYPICAL: PresetModel = m("SEXS", "0.1 10.0 100.0 0.1 0.0 4.0");

const KUNDUR: &str = "P. Kundur, Power System Stability and Control, McGraw-Hill, 1994";
const ANDERSON_FOUAD: &str =
    "P. M. Anderson, A. A. Fouad, Power System Control and Stability, 2nd ed., IEEE Press / Wiley, 2003, Table 2.1 (WSCC 3-machine 9-bus system); also P. W. Sauer, M. A. Pai, Power System Dynamics and Stability, Prentice Hall, 1998, Example 7.1";

pub const UNIT_PRESETS: &[UnitPreset] = &[
    UnitPreset {
        id: "steam_kundur_555",
        kind: "steam",
        label: "Steam, 555 MVA two-pole turbo-generator (Kundur Ex. 3.1)",
        rated_mva: Some(555.0),
        source: KUNDUR,
        notes: "Machine: Kundur Example 3.1 (Xd 1.81, Xq 1.76, X'd 0.30, X'q 0.65, X''d 0.23, Xl 0.15, T'do 8.0, T''do 0.03, T'qo 1.0, T''qo 0.07, H 3.5); GENROU takes X''q = X''d; S(1.0) 0.12 and S(1.2) 0.41 from the example's saturation curve (Asat 0.031, Bsat 6.93, psiT1 0.8). Exciter: the thyristor exciter of Example 13.2 (KA 200, TR 0.015, Efd -6.4..7.0) as SEXS without transient gain reduction. Stabilizer: the PSS of Example 13.2 (K 9.5, Tw 1.4, T1 0.154, T2 0.033) as IEEEST, output limits +-0.1 assumed. Governor: typical TGOV1 (assumed).",
        machine: m("GENROU", "8.0 0.03 1.0 0.07 3.5 0.0 1.81 1.76 0.30 0.65 0.23 0.15 0.12 0.41"),
        exciter: Some(m("SEXS", "1.0 1.0 200.0 0.015 -6.4 7.0")),
        governor: Some(TGOV1_TYPICAL),
        stabilizer: Some(m("IEEEST", "1 0 0.0 0.0 0.0 0.0 0.0 0.0 0.154 0.033 1.0 1.0 1.4 1.4 9.5 0.1 -0.1 0.0 0.0")),
    },
    UnitPreset {
        id: "steam_kundur_two_area",
        kind: "steam",
        label: "Steam, 900 MVA unit of the two-area system (Kundur Ex. 12.6)",
        rated_mva: Some(900.0),
        source: KUNDUR,
        notes: "Machine: Kundur Example 12.6, units G1/G2 (Xd 1.8, Xq 1.7, X'd 0.3, X'q 0.55, X''d 0.25, Xl 0.2, T'do 8.0, T''do 0.03, T'qo 0.4, T''qo 0.05, H 6.5); S(1.0) 0.04 and S(1.2) 0.22 from Asat 0.015, Bsat 9.6, psiT1 0.9. Exciter: thyristor exciter KA 200, TR 0.01 as SEXS (limits assumed). Stabilizer: the example's PSS (K 20, Tw 10, T1 0.05, T2 0.02, T3 3.0, T4 5.4) as IEEEST, limits assumed. Governor: the example holds mechanical power constant; typical TGOV1 added (assumed).",
        machine: m("GENROU", "8.0 0.03 0.4 0.05 6.5 0.0 1.8 1.7 0.3 0.55 0.25 0.2 0.04 0.22"),
        exciter: Some(m("SEXS", "1.0 1.0 200.0 0.01 -5.0 5.0")),
        governor: Some(TGOV1_TYPICAL),
        stabilizer: Some(m("IEEEST", "1 0 0.0 0.0 0.0 0.0 0.0 0.0 0.05 0.02 3.0 5.4 10.0 10.0 20.0 0.2 -0.2 0.0 0.0")),
    },
    UnitPreset {
        id: "steam_wscc_192",
        kind: "steam",
        label: "Steam, 192 MVA (WSCC 9-bus, generator 2)",
        rated_mva: Some(192.0),
        source: ANDERSON_FOUAD,
        notes: "Machine: two-axis data of generator 2 converted from 100 MVA to 192 MVA (Xd 1.720, Xq 1.660, X'd 0.230, X'q 0.378, Xl 0.100, T'do 6.0, T'qo 0.535, H 3.33). Subtransient time constants and X''d 0.18, saturation: typical values (assumed). Exciter and governor: typical SEXS and TGOV1 (assumed; the source's IEEE type 1 exciter is not in the library).",
        machine: m("GENROU", "6.0 0.03 0.535 0.05 3.33 0.0 1.720 1.660 0.230 0.378 0.18 0.100 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(TGOV1_TYPICAL),
        stabilizer: None,
    },
    UnitPreset {
        id: "steam_wscc_128",
        kind: "steam",
        label: "Steam, 128 MVA (WSCC 9-bus, generator 3)",
        rated_mva: Some(128.0),
        source: ANDERSON_FOUAD,
        notes: "Machine: two-axis data of generator 3 converted from 100 MVA to 128 MVA (Xd 1.680, Xq 1.610, X'd 0.232, X'q 0.320, Xl 0.095, T'do 5.89, T'qo 0.6, H 2.35). Subtransient time constants and X''d 0.18, saturation: typical values (assumed). Exciter and governor: typical SEXS and TGOV1 (assumed).",
        machine: m("GENROU", "5.89 0.03 0.6 0.05 2.35 0.0 1.680 1.610 0.232 0.320 0.18 0.095 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(TGOV1_TYPICAL),
        stabilizer: None,
    },
    UnitPreset {
        id: "hydro_wscc_247",
        kind: "hydro",
        label: "Hydro, 247.5 MVA salient pole (WSCC 9-bus, generator 1)",
        rated_mva: Some(247.5),
        source: ANDERSON_FOUAD,
        notes: "Machine: data of generator 1 converted from 100 MVA to 247.5 MVA (Xd 0.361, Xq 0.240, X'd 0.150, Xl 0.083, T'do 8.96, H 9.55). X''d 0.12, T''do 0.05, T''qo 0.05 and saturation: typical values (assumed). Governor: HYGOV with water starting time 1.0 s (assumed) and temporary droop tuned per Kundur, Sec. 9.1.4 (r 0.12, Tr 5.0 s for H 9.55). Exciter: typical SEXS (assumed).",
        machine: m("GENSAL", "8.96 0.05 0.05 9.55 0.0 0.361 0.240 0.150 0.12 0.083 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(m("HYGOV", "0.05 0.12 5.0 0.05 0.5 0.2 1.0 0.0 1.0 1.2 0.5 0.08")),
        stabilizer: None,
    },
    UnitPreset {
        id: "hydro_high_head",
        kind: "hydro",
        label: "Hydro, high head (water starting time 0.8 s)",
        rated_mva: None,
        source: "Machine: typical salient-pole hydro values within the ranges of Kundur (1994), Table 4.2, and IEEE Std 1110-2019; governor: HYGOV with temporary droop tuned per Kundur (1994), Sec. 9.1.4: RT = [2.3 - 0.15 (TW - 1)] TW / TM, TR = [5 - 0.5 (TW - 1)] TW, TM = 2H",
        notes: "Typical, not a specific unit: GENSAL Xd 1.0, Xq 0.65, X'd 0.3, X''d 0.23, Xl 0.15, T'do 5.0, T''do 0.05, T''qo 0.06, H 3.0. HYGOV TW 0.8 s, r 0.31, Tr 4.1 s, R 0.05. Exciter: typical SEXS.",
        machine: m("GENSAL", "5.0 0.05 0.06 3.0 0.0 1.0 0.65 0.3 0.23 0.15 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(m("HYGOV", "0.05 0.31 4.1 0.05 0.5 0.2 1.0 0.0 0.8 1.2 0.5 0.08")),
        stabilizer: None,
    },
    UnitPreset {
        id: "hydro_medium_head",
        kind: "hydro",
        label: "Hydro, medium head (water starting time 1.5 s)",
        rated_mva: None,
        source: "As hydro_high_head: Kundur (1994), Table 4.2 and Sec. 9.1.4; IEEE Std 1110-2019",
        notes: "Typical, not a specific unit: the same machine; HYGOV TW 1.5 s, r 0.56, Tr 7.1 s, R 0.05.",
        machine: m("GENSAL", "5.0 0.05 0.06 3.0 0.0 1.0 0.65 0.3 0.23 0.15 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(m("HYGOV", "0.05 0.56 7.1 0.05 0.5 0.2 1.0 0.0 1.5 1.2 0.5 0.08")),
        stabilizer: None,
    },
    UnitPreset {
        id: "hydro_low_head",
        kind: "hydro",
        label: "Hydro, low head (water starting time 3.0 s)",
        rated_mva: None,
        source: "As hydro_high_head: Kundur (1994), Table 4.2 and Sec. 9.1.4; IEEE Std 1110-2019",
        notes: "Typical, not a specific unit: the same machine; HYGOV TW 3.0 s, r 1.0, Tr 12 s, R 0.05.",
        machine: m("GENSAL", "5.0 0.05 0.06 3.0 0.0 1.0 0.65 0.3 0.23 0.15 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(m("HYGOV", "0.05 1.0 12.0 0.05 0.5 0.2 1.0 0.0 3.0 1.2 0.5 0.08")),
        stabilizer: None,
    },
    UnitPreset {
        id: "gas_turbine",
        kind: "gas",
        label: "Gas turbine, single shaft (GAST)",
        rated_mva: None,
        source: "Governor: GAST, the simplified gas turbine model after W. I. Rowen, \"Simplified mathematical representations of heavy-duty gas turbines\", ASME J. Eng. Power 105(4), 1983, as in the PSS/E model library; machine: typical round-rotor values within the ranges of Kundur (1994), Table 4.2",
        notes: "Typical, not a specific unit: GENROU Xd 1.8, Xq 1.75, X'd 0.3, X'q 0.6, X''d 0.25, Xl 0.15, H 4.0; GAST R 0.05, T1 0.4, T2 0.1, T3 3.0, AT 1.0, KT 2.0, VMAX 1.0, VMIN 0. Exciter: typical SEXS.",
        machine: m("GENROU", "7.0 0.03 0.75 0.05 4.0 0.0 1.8 1.75 0.3 0.6 0.25 0.15 0.1 0.3"),
        exciter: Some(SEXS_TYPICAL),
        governor: Some(m("GAST", "0.05 0.4 0.1 3.0 1.0 2.0 1.0 0.0 0.0")),
        stabilizer: None,
    },
    UnitPreset {
        id: "inverter_wecc",
        kind: "inverter",
        label: "Inverter-based resource (WECC REGCA1 + REECB1, typical)",
        rated_mva: None,
        source: "WECC Renewable Energy Modeling Task Force, generic renewable models (REGC_A, REEC_B), typical data of the WECC solar plant modeling guideline",
        notes: "Typical values. Without a REPCA1 plant controller RMS holds its load flow output as a constant current.",
        machine: m("REGCA1", ""),
        exciter: None,
        governor: None,
        stabilizer: None,
    },
];

/// The presets as JSON.
#[must_use]
pub fn unit_presets_json() -> String {
    serde_json::to_string(UNIT_PRESETS).unwrap_or_else(|_| "[]".into())
}

/// A preset by id.
#[must_use]
pub fn unit_preset(id: &str) -> Option<&'static UnitPreset> {
    UNIT_PRESETS
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(id.trim()))
}
