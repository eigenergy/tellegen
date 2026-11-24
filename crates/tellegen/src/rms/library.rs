//! The dynamic model library as DYR records with typical parameters, and
//! the edits that turn a static (power flow) case dynamic: give a generating
//! unit a machine, exciter, governor, stabilizer or an inverter model, swap
//! one for another, or take it out. Records are written as PSS/E DYR text,
//! so a case built in the browser saves, reloads and opens in other tools.
//!
//! @block Model library

use crate::case_edit::UnitKey;
use serde::{Deserialize, Serialize};

use powerio::{BalancedNetwork, DynamicCase, DynamicRecord};

/// One model of the library: role, PSS/E name, what it is, typical
/// parameters (PSS/E order, on the machine base), and how the engines run it.
// @block Library model
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LibraryModel {
    /// `machine`, `exciter`, `governor`, `stabilizer`, `inverter` (a whole
    /// inverter-based unit), or one part of an inverter-based unit:
    /// `converter`, `electrical` (electrical control) or `plant` (plant
    /// controller).
    pub role: &'static str,
    pub name: &'static str,
    pub label: &'static str,
    /// The RMS engine's representation.
    pub engines: &'static str,
    #[serde(skip)]
    params: &'static str,
}

pub(super) const LIBRARY: &[LibraryModel] = &[
    LibraryModel {
        role: "machine",
        name: "GENROU",
        label: "Round-rotor machine (steam, gas)",
        engines: "RMS",
        params: "7.0 0.03 0.75 0.05 4.0 0.0 1.8 1.75 0.3 0.6 0.25 0.15 0.1 0.3",
    },
    LibraryModel {
        role: "machine",
        name: "GENROE",
        label: "Round-rotor machine, exponential saturation",
        engines: "RMS",
        params: "7.0 0.03 0.75 0.05 4.0 0.0 1.8 1.75 0.3 0.6 0.25 0.15 0.1 0.3",
    },
    LibraryModel {
        role: "machine",
        name: "GENSAL",
        label: "Salient-pole machine (hydro)",
        engines: "RMS",
        params: "5.0 0.05 0.1 3.0 0.0 1.0 0.6 0.3 0.25 0.15 0.1 0.3",
    },
    LibraryModel {
        role: "machine",
        name: "GENSAE",
        label: "Salient-pole machine, exponential saturation",
        engines: "RMS",
        params: "5.0 0.05 0.1 3.0 0.0 1.0 0.6 0.3 0.25 0.15 0.1 0.3",
    },
    LibraryModel {
        role: "machine",
        name: "GENCLS",
        label: "Classical machine (constant voltage behind X'd)",
        engines: "RMS",
        params: "4.0 0.0",
    },
    LibraryModel {
        role: "exciter",
        name: "SEXS",
        label: "Simplified excitation system",
        engines: "RMS",
        params: "0.1 10.0 100.0 0.1 0.0 4.0",
    },
    LibraryModel {
        role: "exciter",
        name: "ESAC1A",
        label: "IEEE AC1A rotating rectifier exciter",
        engines: "RMS",
        // The exciters of PowerSimulationsDynamics.jl's IEEE 14-bus benchmark
        // (`test/benchmarks/psse/GAST/dyn_data.dyr`, BSD-3-Clause), unchanged:
        // the compiled model divides by TB, so the lead-lag cannot be
        // bypassed with TB = 0.
        params: "0.001 0.2 0.001 400.0 0.02 10.0 -10.0 0.8 0.03 1.0 0.2 0.38 1.0 3.14 0.03 4.18 0.1 7.32 -7.32",
    },
    LibraryModel {
        role: "exciter",
        name: "EXST1",
        label: "IEEE ST1 static exciter",
        engines: "RMS",
        params: "0.02 0.1 -0.1 1.0 10.0 200.0 0.02 6.0 -6.0 0.05 0.0 1.0",
    },
    // Experimental: read by tellegen
    // itself (rms/ext_models.rs) and run as block graphs.
    //
    // The ESST1A of PowerSystems.jl 5.12.5's `test/test_dynamic_generator.jl`
    // (BSD-3-Clause), unchanged.
    LibraryModel {
        role: "exciter",
        name: "ESST1A",
        label: "IEEE ST1A static (bus-fed thyristor) exciter",
        engines: "RMS (block graph; validated against PowerSimulationsDynamics.jl 0.16.2)",
        params: "1 1 0.0 99.0 -99.0 2.5 13.25 0.0 0.0 200.0 0.1 9.5 -9.5 9.5 -9.5 0.0 0.0 1.0 0.0 999.0",
    },
    // No published ESAC6A set: the regulator of the ESST1A above (KA, TA,
    // TC/TB, VAMAX/VAMIN), no regulator lead (TK = 0), and the transducer, AC
    // exciter and VRMAX/VRMIN of the ESAC1A above; the exciter field current
    // limiter off (KH = 0).
    LibraryModel {
        role: "exciter",
        name: "ESAC6A",
        label: "IEEE AC6A brushless AC exciter",
        engines: "RMS (block graph; validated against PowerSimulationsDynamics.jl 0.16.2 with its ESAC1A exciter equations)",
        params: "0.001 200.0 0.1 0.0 13.25 2.5 9.5 -9.5 7.32 -7.32 0.8 99.0 0.0 99.0 0.0 0.0 0.2 0.38 1.0 3.14 0.03 4.18 0.1",
    },
    LibraryModel {
        role: "stabilizer",
        name: "PSS2B",
        label: "IEEE PSS2B dual-input (speed, power) stabilizer",
        engines: "RMS (block graph; validated against PowerSimulationsDynamics.jl 0.16.2)",
        params: "1 0 3 0 5 1 1.5 1.5 0.0 1.5 0.0 1.5 0.1875 1.0 0.5 0.1 2.0 0.59451 0.0447 0.59451 0.0447 1.0 1.0 99.0 -99.0 99.0 -99.0 0.1 -0.1",
    },
    // OELIT and UELPQ are tellegen's own models; there are no published sets
    // and these values are illustrative. OELIT: rated field current 2.4 (the
    // unsaturated field current of a round-rotor unit with Xd = 1.8 pu at
    // rated MVA, 1 pu voltage and 0.9 power factor), the IEEE C50.13
    // short-time field capability `t = 33.75 / ((I/Irated)^2 - 1)` (pickup
    // and continuous limit at rated current, C = 33.75 s, instantaneous limit
    // its 10 s point 2.09), TCOOL 1200 s. UELPQ: the limit at P = 0 at 90 %
    // of the steady-state stability limit -V^2/Xd for Xd = 1.8 pu (-0.5),
    // rising by 0.1 per pu of P, scaled with Vt^2 as that limit is. Both act
    // through KP 0.05, KI 0.5 and an output range of 0.5 pu.
    LibraryModel {
        role: "oel",
        name: "OELIT",
        label: "Over-excitation limiter, inverse time (summing point)",
        engines: "RMS (block graph, part of the exciter)",
        params: "2.4 1.0 1.0 2.09 33.75 1200.0 0.05 0.5 -0.5",
    },
    LibraryModel {
        role: "uel",
        name: "UELPQ",
        label: "Under-excitation limiter, P-Q line (summing point)",
        engines: "RMS (block graph, part of the exciter)",
        params: "-0.5 0.1 2 0.05 0.5 0.5",
    },
    LibraryModel {
        role: "governor",
        name: "TGOV1",
        label: "Steam turbine-governor",
        engines: "RMS",
        params: "0.05 0.5 1.0 0.0 2.1 7.0 0.0",
    },
    LibraryModel {
        role: "governor",
        name: "GAST",
        label: "Gas turbine-governor",
        engines: "RMS",
        params: "0.05 0.4 0.1 3.0 1.0 2.0 1.0 0.0 0.0",
    },
    LibraryModel {
        role: "governor",
        name: "HYGOV",
        label: "Hydro turbine-governor",
        engines: "RMS",
        params: "0.05 0.3 5.0 0.05 0.5 0.2 1.0 0.0 1.0 1.2 0.5 0.08",
    },
    LibraryModel {
        role: "stabilizer",
        name: "IEEEST",
        label: "IEEE standard power system stabilizer (speed input)",
        engines: "RMS",
        params: "1 0 0.0 0.0 0.0 0.0 0.0 0.0 0.15 0.05 0.15 0.05 2.0 2.0 10.0 0.1 -0.1 0.0 0.0",
    },
    LibraryModel {
        role: "inverter",
        name: "REGCA1",
        label: "Inverter-based resource (WECC REGCA1 + REECB1)",
        engines: "RMS: simulated with a REPCA1 plant controller (REGCA1 + REECB1 + REPCA1); without one, a constant current at its power flow output",
        params: REGCA1,
    },
    // The parts of an inverter-based unit, each swapped on its own.
    LibraryModel {
        role: "converter",
        name: "REGCA1",
        label: "Converter interface (WECC REGCA1)",
        engines: "RMS: part of a REGCA1 + REECB1 + REPCA1 plant",
        params: REGCA1,
    },
    LibraryModel {
        role: "electrical",
        name: "REECB1",
        label: "Electrical control (WECC REECB1)",
        engines: "RMS: part of a REGCA1 + REECB1 + REPCA1 plant",
        params: REECB1,
    },
    LibraryModel {
        role: "plant",
        name: "REPCA1",
        label: "Plant controller (WECC REPCA1)",
        engines: "RMS: with REGCA1 + REECB1 a simulated plant; without a plant controller the unit is a constant current at its power flow output",
        params: REPCA1,
    },
];

/// The converter of the inverter chain.
const REGCA1: &str = "1 0.02 10.0 0.9 0.5 1.22 1.2 0.8 0.4 1.3 0.02 0.7 999.0 -999.0 0.7";

/// The electrical controls the inverter chain's REGCA1 comes with: the five
/// flags (IREG, PFFLAG, VFLAG, QFLAG, PQFLAG), then the 25 constants.
const REECB1: &str =
    "0 0 0 0 0 0.9 1.1 0.0 -0.05 0.05 0.0 1.1 -1.1 0.0 0.02 0.436 -0.436 1.1 0.9 0.0 0.1 0.0 1.0 0.02 1.0 -1.0 1.0 0.0 1.1 0.02";

/// A plant controller regulating its own terminal, with no monitored branch
/// (the only form the engine simulates): the values of the WECC 240 test
/// case's plants.
const REPCA1: &str = "0 0 0 '0' 0 0 1 0.02 18.0 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 -1.0 1.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0";

/// The roles of the parts of an inverter-based unit, in chain order.
pub(crate) const INVERTER_PARTS: [&str; 3] = ["converter", "electrical", "plant"];

/// The part an inverter record plays: `converter` (REGC*), `electrical`
/// (REEC*), `plant` (REPC*), or `inverter` for another inverter record
/// (wind turbine models, ...).
pub(crate) fn inverter_part(name: &str) -> &'static str {
    let n = name.trim().to_ascii_uppercase();
    if n.starts_with("REGC") {
        "converter"
    } else if n.starts_with("REEC") {
        "electrical"
    } else if n.starts_with("REPC") {
        "plant"
    } else {
        "inverter"
    }
}

/// The model library (the roles and models a unit can get).
#[must_use]
pub fn library() -> &'static [LibraryModel] {
    LIBRARY
}

/// Inverter-based resource models (records tellegen keeps but does not
/// simulate in RMS).
pub(crate) fn is_inverter(name: &str) -> bool {
    let n = name.trim().to_ascii_uppercase();
    n.starts_with("REGC") || n.starts_with("REEC") || n.starts_with("REPC") || n.starts_with("WT")
}

/// The role of a DYR record: a typed model's role, or `inverter`.
fn role_of(rec: &DynamicRecord) -> Option<&'static str> {
    if is_inverter(rec.model_name()) {
        return Some(inverter_part(rec.model_name()));
    }
    // Experimental: models tellegen reads itself.
    if let Some(role) = super::ext_models::ext_role(rec.model_name()) {
        return Some(role);
    }
    Some(match powerio::typed_model(rec).ok()?.role() {
        powerio::ModelRole::Machine => "machine",
        powerio::ModelRole::Exciter => "exciter",
        powerio::ModelRole::Governor => "governor",
        powerio::ModelRole::Stabilizer => "stabilizer",
        _ => return None,
    })
}

fn quote_id(id: &str) -> String {
    format!("'{}'", id.trim())
}

/// The DYR record text of a library model for the unit `(bus, id)`.
pub(super) fn record(model: &LibraryModel, bus: usize, id: &str) -> String {
    format!("{bus} '{}' {} {} /", model.name, quote_id(id), model.params)
}

/// The records of `(bus, id)` (by the DYR key and device id).
fn unit_records<'a>(dyr: &'a DynamicCase, bus: usize, id: &str) -> Vec<&'a DynamicRecord> {
    dyr.records()
        .iter()
        .filter(|r| {
            r.bus_id().is_some_and(|b| b as usize == bus)
                && r.device_id().unwrap_or("1").trim() == id.trim()
        })
        .collect()
}

/// `text` without the record that starts on `line`.
pub(crate) fn remove_record(text: &str, rec: &DynamicRecord) -> Result<String, String> {
    let (at, end) = locate(text, rec)?;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..at]);
    out.push_str(&text[end..]);
    Ok(out)
}

/// The byte span of `rec` in `text`, with its line break.
fn locate(text: &str, rec: &DynamicRecord) -> Result<(usize, usize), String> {
    let raw = rec.raw_record();
    let from = text
        .match_indices('\n')
        .nth(rec.line().saturating_sub(2))
        .map_or(0, |(i, _)| if rec.line() <= 1 { 0 } else { i + 1 });
    let at = text[from..]
        .find(raw)
        .map(|k| from + k)
        .ok_or_else(|| format!("line {}: record text not found", rec.line()))?;
    let mut end = at + raw.len();
    if text[end..].starts_with("\r\n") {
        end += 2;
    } else if text[end..].starts_with('\n') {
        end += 1;
    }
    Ok((at, end))
}

/// `raw` (a record's text) with its bus and device id replaced: the first
/// and third fields.
fn rekey_record(raw: &str, bus: usize, id: &str) -> Option<String> {
    let mut spans = Vec::new();
    let mut i = 0;
    let b = raw.as_bytes();
    while spans.len() < 3 && i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        let start = i;
        if i < b.len() && b[i] == b'\'' {
            i += 1;
            while i < b.len() && b[i] != b'\'' {
                i += 1;
            }
            i += 1;
        } else {
            while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b',' {
                i += 1;
            }
        }
        spans.push((start, i.min(b.len())));
    }
    let [(b0, b1), _, (i0, i1)] = spans[..] else {
        return None;
    };
    Some(format!(
        "{}{bus}{}{}{}",
        &raw[..b0],
        &raw[b1..i0],
        quote_id(id),
        &raw[i1..]
    ))
}

/// DYR records follow the units of an edited case
/// ([`crate::case_edit::EditOutcome`]): renamed units get their new
/// `(bus, id)`, removed units lose their records.
///
/// # Errors
/// DYR text that does not parse.
pub fn dyr_rekey(
    text: &str,
    renames: &[(UnitKey, UnitKey)],
    removed: &[UnitKey],
) -> Result<String, String> {
    let dyr = powerio::parse_dyr(text).map_err(|e| format!("DYR: {e}"))?;
    let key_of = |r: &DynamicRecord| -> Option<UnitKey> {
        Some((
            r.bus_id()? as usize,
            r.device_id().unwrap_or("1").trim().to_string(),
        ))
    };
    let mut spans = Vec::new();
    for r in dyr.records() {
        let Some(key) = key_of(r) else { continue };
        let same = |k: &UnitKey| k.0 == key.0 && k.1.trim() == key.1;
        if removed.iter().any(same) {
            spans.push((locate(text, r)?, None));
        } else if let Some((_, to)) = renames.iter().find(|(from, _)| same(from)) {
            let (at, end) = locate(text, r)?;
            let raw = r.raw_record();
            let new = rekey_record(raw, to.0, &to.1)
                .ok_or_else(|| format!("line {}: record has no unit key", r.line()))?;
            let tail = &text[at + raw.len()..end];
            spans.push(((at, end), Some(format!("{new}{tail}"))));
        }
    }
    spans.sort_by_key(|((at, _), _)| std::cmp::Reverse(*at));
    let mut out = text.to_string();
    for ((at, end), new) in spans {
        out.replace_range(at..end, new.as_deref().unwrap_or(""));
    }
    powerio::parse_dyr(&out).map_err(|e| format!("rekeyed DYR: {e}"))?;
    Ok(out)
}

fn append(text: &mut String, record: &str) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(record);
    text.push('\n');
}

/// Set the model of one role of the unit `(bus, id)`: `Some(name)` replaces
/// that role's record by the library model with typical parameters (or adds
/// it), `None` removes it. An inverter replaces the unit's machine and
/// controls (and a machine its inverter records). The parts of an
/// inverter-based unit (`converter`, `electrical`, `plant`) swap one record
/// of a unit that has a converter; only the plant controller can be removed.
///
/// # Errors
/// An unknown model, a model of another role, or DYR text that does not
/// parse.
// @block Give a unit a model
pub fn dyr_set_model(
    text: &str,
    bus: usize,
    id: &str,
    role: &str,
    model: Option<&str>,
) -> Result<String, String> {
    let chosen = match model {
        Some(name) => {
            let named = |m: &&LibraryModel| m.name.eq_ignore_ascii_case(name);
            let m = LIBRARY
                .iter()
                .filter(named)
                .find(|m| m.role == role)
                .or_else(|| LIBRARY.iter().find(named))
                .ok_or_else(|| format!("model {name} is not in the library"))?;
            if m.role != role {
                return Err(format!(
                    "{} is a {} model, not a {role} model",
                    m.name, m.role
                ));
            }
            Some(m)
        }
        None => None,
    };
    // One part of an inverter-based unit: the unit must be one, and only the
    // plant controller may go (the unit then holds a constant current); the
    // converter and the electrical control are swapped, not removed.
    if INVERTER_PARTS.contains(&role) {
        if chosen.is_none() && role != "plant" {
            return Err(format!(
                "the {role} of an inverter-based unit can be swapped, not removed \
                 (make the unit static to remove the inverter)"
            ));
        }
        let dyr = powerio::parse_dyr(text).map_err(|e| format!("DYR: {e}"))?;
        if !unit_records(&dyr, bus, id)
            .iter()
            .any(|r| role_of(r) == Some("converter"))
        {
            return Err(format!(
                "unit {bus} {} has no inverter (REGCA1): give it one first",
                quote_id(id)
            ));
        }
    }
    // Roles whose records go: the role itself; for an inverter (or a
    // machine), also the other kind of unit.
    const ALL: &[&str] = &[
        "inverter",
        "converter",
        "electrical",
        "plant",
        "machine",
        "exciter",
        "governor",
        "stabilizer",
    ];
    let drop: &[&str] = match (role, chosen.is_some()) {
        ("inverter", true) => ALL,
        ("inverter", false) => &["inverter", "converter", "electrical", "plant"],
        ("machine", true) => &["machine", "inverter", "converter", "electrical", "plant"],
        ("machine", false) => &["machine", "exciter", "governor", "stabilizer", "oel", "uel"],
        _ => &[],
    };
    let mut out = text.to_string();
    loop {
        let dyr = powerio::parse_dyr(&out).map_err(|e| format!("DYR: {e}"))?;
        let next = unit_records(&dyr, bus, id)
            .into_iter()
            .find(|r| role_of(r).is_some_and(|ro| ro == role || drop.contains(&ro)));
        match next {
            Some(r) => out = remove_record(&out, r)?,
            None => break,
        }
    }
    if let Some(m) = chosen {
        append(&mut out, &record(m, bus, id));
        if m.role == "inverter" {
            append(
                &mut out,
                &format!("{bus} 'REECB1' {} {REECB1} /", quote_id(id)),
            );
        }
    }
    powerio::parse_dyr(&out).map_err(|e| format!("edited DYR: {e}"))?;
    Ok(out)
}

/// Give the unit `(bus, id)` the models of a published unit
/// ([`super::presets::UNIT_PRESETS`]): its records replace the unit's
/// machine, controls and inverter records.
///
/// # Errors
/// An unknown preset, or DYR text that does not parse.
pub fn dyr_apply_preset(text: &str, bus: usize, id: &str, preset: &str) -> Result<String, String> {
    let p =
        super::presets::unit_preset(preset).ok_or_else(|| format!("no unit preset {preset}"))?;
    if p.kind == "inverter" {
        return dyr_set_model(text, bus, id, "inverter", Some("REGCA1"));
    }
    let mut out = dyr_set_model(text, bus, id, "inverter", None)?;
    out = dyr_set_model(&out, bus, id, "machine", None)?;
    for m in [Some(p.machine), p.exciter, p.governor, p.stabilizer]
        .into_iter()
        .flatten()
    {
        append(
            &mut out,
            &format!("{bus} '{}' {} {} /", m.model, quote_id(id), m.params),
        );
    }
    powerio::parse_dyr(&out).map_err(|e| format!("preset {}: {e}", p.id))?;
    Ok(out)
}

/// Which models to give units in [`dyr_dynamize`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DynamizeRequest {
    /// Units to give models; empty: every in-service generator.
    #[serde(default)]
    pub units: Vec<(usize, String)>,
    /// Machine model, or `REGCA1` for an inverter-based resource.
    #[serde(default)]
    pub machine: String,
    /// A unit from published data ([`super::presets::UNIT_PRESETS`]) in
    /// place of `machine` and the controls.
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub exciter: Option<String>,
    #[serde(default)]
    pub governor: Option<String>,
    #[serde(default)]
    pub stabilizer: Option<String>,
    /// Replace units that already have dynamic records (else they stay).
    #[serde(default)]
    pub replace: bool,
}

/// Give the generators of a static case (or of a partly dynamic one) dynamic
/// models with typical parameters.
///
/// # Errors
/// As [`dyr_set_model`].
// @block Make a static case dynamic
pub fn dyr_dynamize(
    network: &BalancedNetwork,
    text: &str,
    request: &DynamizeRequest,
) -> Result<String, String> {
    let ids = powerio::psse_machine_ids(network);
    let mut out = text.to_string();
    for (g, id) in network.generators().iter().zip(&ids) {
        if !g.in_service {
            continue;
        }
        let bus = g.bus.0;
        if !request.units.is_empty() && !request.units.iter().any(|(b, i)| *b == bus && i == id) {
            continue;
        }
        let dyr = powerio::parse_dyr(&out).map_err(|e| format!("DYR: {e}"))?;
        if !request.replace && !unit_records(&dyr, bus, id).is_empty() {
            continue;
        }
        if let Some(preset) = &request.preset {
            out = dyr_apply_preset(&out, bus, id, preset)?;
            continue;
        }
        let inverter = request.machine.eq_ignore_ascii_case("REGCA1");
        out = dyr_set_model(
            &out,
            bus,
            id,
            if inverter { "inverter" } else { "machine" },
            Some(&request.machine),
        )?;
        if inverter {
            continue;
        }
        for (role, model) in [
            ("exciter", &request.exciter),
            ("governor", &request.governor),
            ("stabilizer", &request.stabilizer),
        ] {
            out = dyr_set_model(&out, bus, id, role, model.as_deref())?;
        }
    }
    Ok(out)
}

/// [`dyr_dynamize`] for a stored module, the request as JSON.
///
/// # Errors
/// When the inputs do not parse.
pub fn dyr_dynamize_json(
    module_json: &str,
    dyr_text: &str,
    request_json: &str,
) -> Result<String, String> {
    let module = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?;
    let request: DynamizeRequest = serde_json::from_str(request_json).map_err(|e| e.to_string())?;
    dyr_dynamize(module.value(), dyr_text, &request)
}

/// The library as JSON.
#[must_use]
pub fn library_json() -> String {
    serde_json::to_string(LIBRARY).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/wecc240.rs"
    ));

    /// The parts of an inverter-based unit swap one at a time: the plant
    /// controller goes and comes back (the plant count follows), a new
    /// inverter takes the library's plant controller and is simulated as a
    /// plant, and what cannot be done is refused with a reason.
    #[test]
    fn inverter_parts_swap_and_simulate() {
        use crate::rms::{LoadModel, RmsRequest, RmsSimulation};
        let Some(net) = shipped_wecc240_network() else {
            return;
        };
        let dir = shipped_wecc240().unwrap();
        let base = std::fs::read_to_string(dir.join("case.dyr")).unwrap();
        let plants = |text: &str| -> usize {
            let dyr = powerio::parse_dyr(text).unwrap();
            let res = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance)
                .unwrap()
                .run(&RmsRequest {
                    t_end: 0.2,
                    dt: 0.005,
                    output_step: 0.05,
                    load_model: Some(LoadModel::ConstantImpedance),
                    events: vec![],
                    small_signal: false,
                    record: None,
                })
                .unwrap();
            res.notes
                .iter()
                .find_map(|n| {
                    n.strip_suffix(" renewable plant(s) simulated as REGCA1 + REECB1 + REPCA1")
                        .and_then(|k| k.parse().ok())
                })
                .unwrap_or(0)
        };
        let models = |text: &str, bus: usize, id: &str| -> Vec<String> {
            let dyr = powerio::parse_dyr(text).unwrap();
            unit_records(&dyr, bus, id)
                .iter()
                .map(|r| r.model_name().trim().to_string())
                .collect()
        };
        let n0 = plants(&base);
        assert!(n0 > 0);
        // The plant controller out and back in.
        let without = dyr_set_model(&base, 1032, "S", "plant", None).unwrap();
        assert_eq!(models(&without, 1032, "S"), ["REGCA1", "REECB1"]);
        assert_eq!(plants(&without), n0 - 1);
        let back = dyr_set_model(&without, 1032, "S", "plant", Some("REPCA1")).unwrap();
        assert_eq!(models(&back, 1032, "S"), ["REGCA1", "REECB1", "REPCA1"]);
        assert_eq!(plants(&back), n0);
        // Swapping the electrical control keeps the chain.
        let swapped = dyr_set_model(&back, 1032, "S", "electrical", Some("REECB1")).unwrap();
        assert_eq!(models(&swapped, 1032, "S"), ["REGCA1", "REPCA1", "REECB1"]);
        assert_eq!(plants(&swapped), n0);
        // A machine made an inverter, with the library's plant controller.
        let inv = dyr_set_model(&base, 1034, "G", "inverter", Some("REGCA1")).unwrap();
        let inv = dyr_set_model(&inv, 1034, "G", "plant", Some("REPCA1")).unwrap();
        assert_eq!(models(&inv, 1034, "G"), ["REGCA1", "REECB1", "REPCA1"]);
        assert_eq!(plants(&inv), n0 + 1);
        // Refused: removing the converter or the electrical control, a part
        // on a machine, an inverter part in another role.
        for (bus, id, role, model) in [
            (1032, "S", "converter", None),
            (1032, "S", "electrical", None),
            (1032, "C", "plant", Some("REPCA1")),
            (1032, "S", "plant", Some("REECB1")),
        ] {
            assert!(
                dyr_set_model(&base, bus, id, role, model).is_err(),
                "{bus} {id} {role} {model:?}"
            );
        }
        // A whole inverter still replaces every part; removing it takes all.
        let none = dyr_set_model(&base, 1032, "S", "inverter", None).unwrap();
        assert!(models(&none, 1032, "S").is_empty());
    }

    /// Every library record parses through powerio's schema (typed models)
    /// with the arity the engine expects.
    #[test]
    fn library_records_parse() {
        for m in LIBRARY {
            let text = record(m, 1, "1");
            let dyr = powerio::parse_dyr(&text).unwrap();
            let r = &dyr.records()[0];
            if super::super::ext_models::ext_role(m.name).is_some() {
                assert_eq!(role_of(r), Some(m.role), "{}", m.name);
                assert_eq!(
                    r.parameters().len(),
                    super::super::dyr_params::param_count(m.name).unwrap_or(0),
                    "{}",
                    m.name
                );
            } else if m.role == "inverter" {
                assert!(is_inverter(r.model_name()));
            } else if INVERTER_PARTS.contains(&m.role) {
                assert!(is_inverter(r.model_name()), "{}", m.name);
                assert_eq!(role_of(r), Some(m.role), "{}", m.name);
                assert_eq!(
                    r.parameters().len(),
                    super::super::dyr_params::param_count(m.name).unwrap_or(0),
                    "{}",
                    m.name
                );
            } else {
                let typed = powerio::typed_model(r).unwrap_or_else(|e| panic!("{}: {e}", m.name));
                assert_eq!(role_of(r), Some(m.role), "{}", m.name);
                assert_eq!(typed.name(), m.name);
            }
        }
    }

    /// A static case made dynamic initializes and runs in RMS; models swap
    /// and come out again; an inverter replaces the machine.
    #[test]
    fn dynamize_swap_remove_and_simulate() {
        use crate::rms::{LoadModel, RmsEvent, RmsRequest, RmsSimulation};
        let Some(net) = shipped_wecc240_network() else {
            return;
        };
        let req = DynamizeRequest {
            machine: "GENROU".into(),
            exciter: Some("SEXS".into()),
            governor: Some("TGOV1".into()),
            stabilizer: Some("IEEEST".into()),
            ..DynamizeRequest::default()
        };
        let text = dyr_dynamize(&net, "", &req).unwrap();
        let dyr = powerio::parse_dyr(&text).unwrap();
        let units = net.generators().iter().filter(|g| g.in_service).count();
        assert_eq!(dyr.records().len(), 4 * units, "{text}");
        let request = RmsRequest {
            t_end: 2.0,
            dt: 0.005,
            output_step: 0.01,
            load_model: Some(LoadModel::ConstantImpedance),
            events: vec![
                RmsEvent::BusFault {
                    time: 0.5,
                    bus: 1101,
                    r: 0.0,
                    x: 0.05,
                },
                RmsEvent::ClearFault {
                    time: 0.6,
                    bus: 1101,
                },
            ],
            small_signal: false,
            record: None,
        };
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        let res = sim.run(&request).unwrap();
        assert!(res.channels["omega:1032#C"]
            .iter()
            .all(|w| (w - 1.0).abs() < 0.05));
        // Swap an exciter, take a stabilizer out, make another unit an inverter.
        let text = dyr_set_model(&text, 1032, "C", "exciter", Some("ESAC1A")).unwrap();
        let text = dyr_set_model(&text, 1032, "C", "stabilizer", None).unwrap();
        let text = dyr_set_model(&text, 1034, "G", "inverter", Some("REGCA1")).unwrap();
        let dyr = powerio::parse_dyr(&text).unwrap();
        let unit1: Vec<String> = unit_records(&dyr, 1032, "C")
            .iter()
            .map(|r| r.model_name().to_string())
            .collect();
        assert_eq!(unit1, ["GENROU", "TGOV1", "ESAC1A"]);
        let unit2: Vec<String> = unit_records(&dyr, 1034, "G")
            .iter()
            .map(|r| r.model_name().to_string())
            .collect();
        assert_eq!(unit2, ["REGCA1", "REECB1"]);
        RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance)
            .unwrap()
            .run(&request)
            .unwrap();
    }

    /// A generator added by hand gets dynamic models, moves to another bus
    /// with its records, and the case runs a fault in RMS.
    #[test]
    fn added_unit_moves_with_its_records() {
        use crate::case_edit::{apply_case_edit, CaseEdit};
        use crate::rms::{LoadModel, RmsEvent, RmsRequest, RmsSimulation};
        let Some(mut net) = shipped_wecc240_network() else {
            return;
        };
        let req = DynamizeRequest {
            machine: "GENROU".into(),
            exciter: Some("SEXS".into()),
            governor: Some("TGOV1".into()),
            ..DynamizeRequest::default()
        };
        let text = dyr_dynamize(&net, "", &req).unwrap();
        let add = CaseEdit::AddGenerator {
            bus: 1002,
            pg: 20.0,
            pmax: None,
            qmax: None,
            qmin: None,
            vg: None,
            mbase: None,
        };
        let out = apply_case_edit(&mut net, &add).unwrap();
        let (bus, id) = out.unit.unwrap();
        let mut text = text;
        for (role, model) in [
            ("machine", "GENROU"),
            ("exciter", "ESAC1A"),
            ("governor", "GAST"),
        ] {
            text = dyr_set_model(&text, bus, &id, role, Some(model)).unwrap();
        }
        let out = apply_case_edit(
            &mut net,
            &CaseEdit::MoveGenerator {
                bus,
                id: id.clone(),
                to: 1004,
            },
        )
        .unwrap();
        let text = dyr_rekey(&text, &out.renames, &out.removed).unwrap();
        let dyr = powerio::parse_dyr(&text).unwrap();
        assert_eq!(unit_records(&dyr, 1004, "1").len(), 3, "{text}");
        assert!(unit_records(&dyr, 1002, "1").is_empty(), "{text}");
        let request = RmsRequest {
            t_end: 1.5,
            dt: 0.005,
            output_step: 0.01,
            load_model: Some(LoadModel::ConstantImpedance),
            events: vec![
                RmsEvent::BusFault {
                    time: 0.5,
                    bus: 1101,
                    r: 0.0,
                    x: 0.05,
                },
                RmsEvent::ClearFault {
                    time: 0.6,
                    bus: 1101,
                },
            ],
            small_signal: false,
            record: None,
        };
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        let res = sim.run(&request).unwrap();
        let w = &res.channels["omega:1004"];
        assert!(w.iter().all(|w| (w - 1.0).abs() < 0.05));
        // Removing the unit removes its records.
        let out = apply_case_edit(
            &mut net,
            &CaseEdit::RemoveGenerator {
                bus: 1004,
                id: "1".into(),
            },
        )
        .unwrap();
        let text = dyr_rekey(&text, &out.renames, &out.removed).unwrap();
        let dyr = powerio::parse_dyr(&text).unwrap();
        assert!(unit_records(&dyr, 1004, "1").is_empty());
        RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    }

    /// Every published unit parses with the arity its models need,
    /// initializes on WECC 240 and rides through a fault.
    #[test]
    fn presets_parse_initialize_and_simulate() {
        use crate::rms::presets::UNIT_PRESETS;
        use crate::rms::{LoadModel, RmsEvent, RmsRequest, RmsSimulation};
        let Some(net) = shipped_wecc240_network() else {
            return;
        };
        let base = dyr_dynamize(
            &net,
            "",
            &DynamizeRequest {
                machine: "GENROU".into(),
                exciter: Some("SEXS".into()),
                governor: Some("TGOV1".into()),
                ..DynamizeRequest::default()
            },
        )
        .unwrap();
        for p in UNIT_PRESETS {
            let text = dyr_apply_preset(&base, 1032, "C", p.id).unwrap();
            let dyr = powerio::parse_dyr(&text).unwrap();
            for r in unit_records(&dyr, 1032, "C") {
                if !is_inverter(r.model_name()) {
                    powerio::typed_model(r)
                        .unwrap_or_else(|e| panic!("{}: {} {e}", p.id, r.model_name()));
                }
            }
            let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance)
                .unwrap_or_else(|e| panic!("{}: {e}", p.id));
            let res = sim
                .run(&RmsRequest {
                    t_end: 3.0,
                    dt: 0.005,
                    output_step: 0.01,
                    load_model: Some(LoadModel::ConstantImpedance),
                    events: vec![
                        RmsEvent::BusFault {
                            time: 0.5,
                            bus: 1101,
                            r: 0.0,
                            x: 0.05,
                        },
                        RmsEvent::ClearFault {
                            time: 0.6,
                            bus: 1101,
                        },
                    ],
                    small_signal: false,
                    record: None,
                })
                .unwrap_or_else(|e| panic!("{}: {e}", p.id));
            if p.kind != "inverter" {
                let w = &res.channels["omega:1032#C"];
                assert!(w.iter().all(|w| (w - 1.0).abs() < 0.05), "{}", p.id);
                assert!(
                    (w.last().unwrap() - 1.0).abs() < 0.01,
                    "{} does not settle",
                    p.id
                );
            }
        }
    }
}
