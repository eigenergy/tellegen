//! Dynamic models by rule for the units a case brings no dynamic data for,
//! so every case is one model on all layers. Records the case has are kept.
//!
//! Each unit gets a published unit ([`super::presets`]) or a typical model
//! chain by what the case says about it:
//!
//! - wind and solar: an inverter-based resource (WECC REGCA1 + REECB1);
//! - no active power (P max ≤ 1 MW, or dispatched below 0.5 MW with a
//!   reactive range): a synchronous condenser (GENROU with an SEXS exciter,
//!   no governor);
//! - hydro: the WSCC 247.5 MVA salient-pole unit from 200 MW, else the
//!   medium-head unit;
//! - nuclear: the 555 MVA steam unit (Kundur);
//! - thermal or unknown, by rating (the larger of P max and 90 % of the MVA
//!   base, P max ignored when it is a "no limit" sentinel): from 400 MW the
//!   555 MVA steam unit, from 150 MW the WSCC 192 MVA steam unit, from 50 MW
//!   the WSCC 128 MVA unit, below that a gas turbine (GAST).
//!
//! The values are per unit on each unit's own MVA base, so they scale with
//! the unit.
//!
//! Records the case brings in a model tellegen does not simulate are
//! replaced role by role, so the unit keeps the rest of its data:
//!
//! - an exciter by SEXS with typical data;
//! - a governor by TGOV1 with typical data (HYGOV for a salient-pole
//!   machine);
//! - a stabilizer is dropped;
//! - a machine by the unit's rule above, which replaces all its records.
//!
//! The role of an unknown model comes from its PSS/E name.
//!
//! The completed model is then initialized from the power flow. A unit whose
//! assigned model cannot reach its operating point (typically a dispatch far
//! beyond its MVA base, so the field voltage or the governor limit cannot
//! hold it) falls back to a classical machine (GENCLS, H 3 s, D 0), which
//! always initializes; the reason says so.

use powerio::network::GeneratorEnergySource;
use powerio::BalancedNetwork;
use serde::{Deserialize, Serialize};

use super::library::{dyr_apply_preset, dyr_set_model};

/// What a unit got, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoAssignment {
    pub bus: usize,
    pub machine_id: String,
    /// `steam`, `hydro`, `gas`, `nuclear`, `inverter`, `condenser`.
    pub kind: String,
    /// The published unit, when one was used.
    pub preset: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoDynamics {
    pub dyr: String,
    pub assigned: Vec<AutoAssignment>,
    /// Units whose records the case brought.
    pub kept: usize,
}

fn has_records(text: &str, bus: usize, id: &str) -> Result<bool, String> {
    let dyr = powerio::parse_dyr(text).map_err(|e| format!("DYR: {e}"))?;
    Ok(dyr.records().iter().any(|r| {
        r.bus_id().is_some_and(|b| b as usize == bus)
            && r.device_id().unwrap_or("1").trim() == id.trim()
    }))
}

/// Complete `text` with models by rule for every in-service unit without
/// records.
///
/// # Errors
/// When the DYR text does not parse.
pub fn dyr_auto(network: &BalancedNetwork, text: &str) -> Result<AutoDynamics, String> {
    let ids = powerio::psse_machine_ids(network);
    let mut out = AutoDynamics {
        dyr: text.to_string(),
        ..AutoDynamics::default()
    };
    for (g, id) in network.generators().iter().zip(&ids) {
        if !g.in_service {
            continue;
        }
        let bus = g.bus.0;
        let mut replaced_note: Option<String> = None;
        if has_records(&out.dyr, bus, id)? {
            match substitute_unsupported(&mut out.dyr, bus, id)? {
                Substitution::None => {
                    out.kept += 1;
                    continue;
                }
                Substitution::Roles(note) => {
                    out.kept += 1;
                    out.assigned.push(AutoAssignment {
                        bus,
                        machine_id: id.clone(),
                        kind: "substituted".into(),
                        preset: None,
                        reason: note,
                    });
                    continue;
                }
                // The machine itself is not simulated: the whole unit by rule.
                Substitution::Machine(note) => replaced_note = Some(note),
            }
        }
        // P max far above the base and the dispatch is a "no limit" entry.
        let sentinel = g.pmax > 5.0 * g.mbase.max(g.pg.abs()).max(1.0);
        let rating = if sentinel {
            g.mbase.max(g.pg)
        } else {
            g.pmax.max(0.9 * g.mbase)
        };
        let source = g.energy_source;
        let (kind, preset, reason): (&str, Option<&str>, String) = match source {
            GeneratorEnergySource::Wind | GeneratorEnergySource::Solar => (
                "inverter",
                None,
                format!("{source:?} plant: inverter-based"),
            ),
            _ if g.qmax > 0.0 && (g.pmax <= 1.0 || g.pg.abs() < 0.5) => (
                "condenser",
                None,
                format!(
                    "P = {:.1} MW, Q range {:.0}…{:.0} MVAr: synchronous condenser",
                    g.pg, g.qmin, g.qmax
                ),
            ),
            GeneratorEnergySource::Hydro if rating >= 200.0 => (
                "hydro",
                Some("hydro_wscc_247"),
                format!("hydro, {rating:.0} MW"),
            ),
            GeneratorEnergySource::Hydro => (
                "hydro",
                Some("hydro_medium_head"),
                format!("hydro, {rating:.0} MW"),
            ),
            GeneratorEnergySource::Nuclear => (
                "nuclear",
                Some("steam_kundur_555"),
                "nuclear: large steam unit".into(),
            ),
            _ if rating >= 400.0 => (
                "steam",
                Some("steam_kundur_555"),
                format!("{rating:.0} MW: large steam unit"),
            ),
            _ if rating >= 150.0 => (
                "steam",
                Some("steam_wscc_192"),
                format!("{rating:.0} MW: steam unit"),
            ),
            _ if rating >= 50.0 => (
                "steam",
                Some("steam_wscc_128"),
                format!("{rating:.0} MW: steam unit"),
            ),
            _ => (
                "gas",
                Some("gas_turbine"),
                format!("{rating:.0} MW: gas turbine"),
            ),
        };
        out.dyr = match (kind, preset) {
            (_, Some(p)) => dyr_apply_preset(&out.dyr, bus, id, p)?,
            ("inverter", None) => dyr_set_model(&out.dyr, bus, id, "inverter", Some("REGCA1"))?,
            _ => {
                let t = dyr_set_model(&out.dyr, bus, id, "machine", Some("GENROU"))?;
                dyr_set_model(&t, bus, id, "exciter", Some("SEXS"))?
            }
        };
        out.assigned.push(AutoAssignment {
            bus,
            machine_id: id.clone(),
            kind: kind.into(),
            preset: preset.map(str::to_owned),
            reason: match replaced_note {
                Some(n) => format!("{n}; {reason}"),
                None => reason,
            },
        });
    }
    if !out.assigned.is_empty() {
        fall_back_where_init_fails(network, &mut out)?;
    }
    Ok(out)
}

enum Substitution {
    None,
    /// Controllers replaced; the note says which.
    Roles(String),
    /// The machine model is not simulated; its records were removed.
    Machine(String),
}

/// Whether tellegen simulates a record's model (typed, or an inverter
/// chain).
fn simulated(rec: &powerio::DynamicRecord) -> bool {
    super::library::is_inverter(rec.model_name()) || powerio::typed_model(rec).is_ok()
}

/// The role of a PSS/E model by its name, for models tellegen does not know.
fn guess_role(name: &str) -> &'static str {
    let n = name.trim().to_ascii_uppercase();
    if n.starts_with("GEN") || n.starts_with("CIMTR") || n.starts_with("CDSMS") {
        "machine"
    } else if n.contains("PSS")
        || n.starts_with("STAB")
        || n.starts_with("IEEEST")
        || n.starts_with("ST2CUT")
        || n.starts_with("IVOST")
        || n.starts_with("PTIST")
    {
        "stabilizer"
    } else if n.contains("GOV")
        || n.starts_with("IEEEG")
        || n.starts_with("GAST")
        || n.starts_with("IEESGO")
        || n.starts_with("WSIEG")
        || n.starts_with("WEHG")
        || n.starts_with("WPIDHY")
        || n.starts_with("H6")
        || n.starts_with("TURCZT")
        || n.starts_with("URGS")
        || n.starts_with("CRCMG")
        || n.starts_with("WTGT")
        || n.starts_with("TGOV")
    {
        "governor"
    } else {
        "exciter"
    }
}

/// Replace the unit's records whose models are not simulated.
fn substitute_unsupported(text: &mut String, bus: usize, id: &str) -> Result<Substitution, String> {
    let dyr = powerio::parse_dyr(text).map_err(|e| format!("DYR: {e}"))?;
    let mine: Vec<powerio::DynamicRecord> = dyr
        .records()
        .iter()
        .filter(|r| {
            r.bus_id().is_some_and(|b| b as usize == bus)
                && r.device_id().unwrap_or("1").trim() == id.trim()
        })
        .cloned()
        .collect();
    let missing: Vec<(String, &'static str)> = mine
        .iter()
        .filter(|r| !simulated(r))
        .map(|r| (r.model_name().trim().to_owned(), guess_role(r.model_name())))
        .collect();
    if missing.is_empty() {
        return Ok(Substitution::None);
    }
    if missing.iter().any(|(_, role)| *role == "machine") {
        for r in mine.iter().rev() {
            *text = super::library::remove_record(text, r)?;
        }
        let names: Vec<&str> = missing.iter().map(|(n, _)| n.as_str()).collect();
        return Ok(Substitution::Machine(format!(
            "{} not simulated",
            names.join(", ")
        )));
    }
    // Remove the unsupported records (last first, so spans stay valid).
    let mut lines: Vec<&powerio::DynamicRecord> = mine.iter().filter(|r| !simulated(r)).collect();
    lines.sort_by_key(|r| std::cmp::Reverse(r.line()));
    for r in lines {
        *text = super::library::remove_record(text, r)?;
    }
    let salient = mine.iter().any(|r| {
        r.model_name().trim().eq_ignore_ascii_case("GENSAL")
            || r.model_name().trim().eq_ignore_ascii_case("GENSAE")
    });
    let mut parts = Vec::new();
    for (name, role) in &missing {
        match *role {
            "exciter" => {
                *text = dyr_set_model(text, bus, id, "exciter", Some("SEXS"))?;
                parts.push(format!("{name} → SEXS typical"));
            }
            "governor" => {
                let g = if salient { "HYGOV" } else { "TGOV1" };
                *text = dyr_set_model(text, bus, id, "governor", Some(g))?;
                parts.push(format!("{name} → {g} typical"));
            }
            _ => parts.push(format!("{name} dropped (no stabilizer)")),
        }
    }
    Ok(Substitution::Roles(format!(
        "not simulated: {}",
        parts.join(", ")
    )))
}

/// Initialize the completed model; a generated unit that does not reach its
/// operating point becomes a classical machine.
fn fall_back_where_init_fails(
    network: &BalancedNetwork,
    out: &mut AutoDynamics,
) -> Result<(), String> {
    for _ in 0..out.assigned.len().min(200) {
        let dyr = powerio::parse_dyr(&out.dyr).map_err(|e| format!("DYR: {e}"))?;
        let Err(e) = super::RmsSimulation::new(network, &dyr, super::LoadModel::ConstantImpedance)
        else {
            return Ok(());
        };
        // "generator 312-1: ..." names the unit.
        let Some(k) = out.assigned.iter().position(|a| {
            e.contains(&format!("generator {}-{}:", a.bus, a.machine_id))
                || e.contains(&format!("generator {} '{}'", a.bus, a.machine_id))
        }) else {
            return Ok(());
        };
        let a = &mut out.assigned[k];
        if a.kind == "classical" {
            return Ok(());
        }
        let mut t = dyr_set_model(&out.dyr, a.bus, &a.machine_id, "machine", None)?;
        for role in ["exciter", "governor", "stabilizer", "inverter"] {
            t = dyr_set_model(&t, a.bus, &a.machine_id, role, None)?;
        }
        t.push_str(&format!(
            "{} 'GENCLS' '{}' 3.0 0.0 /\n",
            a.bus, a.machine_id
        ));
        out.dyr = t;
        a.reason = format!("{}; the model did not reach the operating point ({}), so a classical machine (GENCLS, H 3 s)", a.reason, e.split(':').nth(1).unwrap_or("").trim());
        a.kind = "classical".into();
        a.preset = None;
    }
    Ok(())
}

/// [`dyr_auto`] over a stored module, as JSON.
///
/// # Errors
/// When the module or the DYR text does not parse.
pub fn dyr_auto_json(module_json: &str, dyr_text: &str) -> Result<String, String> {
    let module = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?;
    let input = super::DynamicInput::from_text(dyr_text)?;
    let mut auto = dyr_auto(module.value(), &input.dyr)?;
    if !input.models.is_empty() {
        auto.dyr = serde_json::to_string(&super::DynamicInput {
            dyr: auto.dyr,
            models: input.models,
        })
        .map_err(|e| e.to_string())?;
    }
    serde_json::to_string(&auto).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_static_case_becomes_dynamic_by_rule() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/rms_examples/wecc240");
        let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!()
        };
        let auto = dyr_auto(net, "").unwrap();
        for a in &auto.assigned {
            println!(
                "{} '{}': {} {:?} ({})",
                a.bus, a.machine_id, a.kind, a.preset, a.reason
            );
        }
        assert_eq!(
            auto.assigned.len(),
            net.generators().iter().filter(|g| g.in_service).count()
        );
        let dyr = powerio::parse_dyr(&auto.dyr).unwrap();
        let sim =
            super::super::RmsSimulation::new(net, &dyr, super::super::LoadModel::ConstantImpedance)
                .unwrap();
        let r = sim
            .run(&super::super::RmsRequest {
                t_end: 3.0,
                events: vec![
                    super::super::RmsEvent::BusFault {
                        time: 0.5,
                        bus: 1101,
                        r: 0.0,
                        x: 0.05,
                    },
                    super::super::RmsEvent::ClearFault {
                        time: 0.6,
                        bus: 1101,
                    },
                ],
                small_signal: false,
                ..Default::default()
            })
            .unwrap();
        assert!(r.time.len() > 100);
        // Keeps what a case brings: only units without records get models.
        let original = std::fs::read_to_string(dir.join("case.dyr")).unwrap();
        let kept = powerio::parse_dyr(&original).unwrap();
        let with_records: std::collections::BTreeSet<(usize, String)> = kept
            .records()
            .iter()
            .filter_map(|r| {
                Some((
                    r.bus_id()? as usize,
                    r.device_id().unwrap_or("1").trim().to_owned(),
                ))
            })
            .collect();
        let again = dyr_auto(net, &original).unwrap();
        assert!(again
            .assigned
            .iter()
            .all(|a| !with_records.contains(&(a.bus, a.machine_id.trim().to_owned()))));
    }
}
