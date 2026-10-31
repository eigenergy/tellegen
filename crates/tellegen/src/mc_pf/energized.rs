//! Explicit bus-island selection. The portable input always retains the full network.
use std::collections::{BTreeMap, BTreeSet};

use powerio::{PioModule, PioValue};
use powerio_dist::MulticonductorNetwork;
use serde::{Deserialize, Serialize};

pub(crate) const EXTENSION: &str = "tellegen.energized_network";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McExcludedEquipment {
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McExcludedIsland {
    pub buses: Vec<String>,
    /// Includes open boundary switches touching this island. An open switch
    /// between two excluded islands appears in both island records.
    pub equipment: Vec<McExcludedEquipment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McNetworkScope {
    pub excluded_island_count: usize,
    pub excluded_bus_count: usize,
    pub excluded_equipment_count: usize,
    pub excluded_load_count: usize,
}

pub(crate) fn scope(islands: &[McExcludedIsland]) -> Option<McNetworkScope> {
    if islands.is_empty() {
        return None;
    }
    let equipment: BTreeSet<_> = islands
        .iter()
        .flat_map(|i| i.equipment.iter())
        .map(|e| (e.kind.as_str(), e.name.as_str()))
        .collect();
    Some(McNetworkScope {
        excluded_island_count: islands.len(),
        excluded_bus_count: islands.iter().map(|i| i.buses.len()).sum(),
        excluded_equipment_count: equipment.len(),
        excluded_load_count: equipment.iter().filter(|(kind, _)| *kind == "load").count(),
    })
}

/// Determine source coverage using lines, closed switches and all transformer
/// windings, matching PowerIO's bus-level instance check. This does not certify
/// conductor-level solvability. Invalid references are rejected, never hidden.
/// Read-only: no solve selection is recorded. With no source, every island is returned.
pub fn mc_unpowered_islands(
    network: &MulticonductorNetwork,
) -> Result<Vec<McExcludedIsland>, String> {
    let mut indices = BTreeMap::new();
    for (i, bus) in network.buses().iter().enumerate() {
        if indices.insert(bus.id.to_ascii_lowercase(), i).is_some() {
            return Err(format!("duplicate bus identity `{}`", bus.id));
        }
    }
    let index = |bus: &str| {
        indices
            .get(&bus.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| format!("equipment references unknown bus `{bus}`"))
    };
    let mut adjacent = vec![Vec::new(); indices.len()];
    let mut equipment = Vec::<(String, String, Vec<usize>)>::new();
    let mut add =
        |kind: &str, name: &str, buses: Vec<&str>, connected: bool| -> Result<(), String> {
            let nodes = buses
                .into_iter()
                .map(&index)
                .collect::<Result<Vec<_>, _>>()?;
            if connected {
                if let Some(&first) = nodes.first() {
                    for &other in &nodes[1..] {
                        adjacent[first].push(other);
                        adjacent[other].push(first);
                    }
                }
            }
            equipment.push((kind.to_owned(), name.to_owned(), nodes));
            Ok(())
        };
    for e in network.lines() {
        add("line", &e.name, vec![&e.bus_from, &e.bus_to], true)?;
    }
    for e in network.switches() {
        add("switch", &e.name, vec![&e.bus_from, &e.bus_to], !e.open)?;
    }
    for e in network.transformers() {
        add(
            "transformer",
            &e.name,
            e.windings.iter().map(|w| w.bus.as_str()).collect(),
            true,
        )?;
    }
    macro_rules! single {
        ($table:ident, $kind:literal) => {
            for e in network.$table() {
                add($kind, &e.name, vec![&e.bus], false)?;
            }
        };
    }
    single!(loads, "load");
    single!(generators, "generator");
    single!(ibrs, "ibr");
    single!(shunts, "shunt");
    single!(capacitors, "capacitor");
    single!(sources, "source");
    let sources = network
        .sources()
        .iter()
        .map(|s| index(&s.bus))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut component = vec![usize::MAX; indices.len()];
    let mut islands = Vec::new();
    for start in 0..component.len() {
        if component[start] != usize::MAX {
            continue;
        }
        let id = start;
        let mut stack = vec![start];
        component[start] = id;
        let mut nodes = Vec::new();
        let mut powered = false;
        while let Some(node) = stack.pop() {
            nodes.push(node);
            powered |= sources.contains(&node);
            for &next in &adjacent[node] {
                if component[next] == usize::MAX {
                    component[next] = id;
                    stack.push(next);
                }
            }
        }
        if !powered {
            let mut buses: Vec<_> = nodes
                .iter()
                .map(|&i| network.buses()[i].id.clone())
                .collect();
            buses.sort();
            islands.push((
                id,
                McExcludedIsland {
                    buses,
                    equipment: Vec::new(),
                },
            ));
        }
    }
    let by_component: BTreeMap<_, _> = islands
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    for (kind, name, nodes) in equipment {
        let affected: BTreeSet<_> = nodes
            .iter()
            .filter_map(|&n| by_component.get(&component[n]).copied())
            .collect();
        for i in affected {
            islands[i].1.equipment.push(McExcludedEquipment {
                kind: kind.clone(),
                name: name.clone(),
            });
        }
    }
    let mut islands: Vec<_> = islands
        .into_iter()
        .map(|(_, mut i)| {
            i.equipment
                .sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
            i
        })
        .collect();
    islands.sort_by(|a, b| a.buses.cmp(&b.buses));
    Ok(islands)
}

fn require_source(network: &MulticonductorNetwork) -> Result<(), String> {
    if network.sources().is_empty() {
        return Err("cannot solve an energized network without a voltage source".to_owned());
    }
    Ok(())
}

fn select(network: &MulticonductorNetwork, islands: &[McExcludedIsland]) -> MulticonductorNetwork {
    let excluded: BTreeSet<_> = islands
        .iter()
        .flat_map(|i| &i.buses)
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let keep = |bus: &str| !excluded.contains(&bus.to_ascii_lowercase());
    let mut selected = network.clone();
    selected.buses_mut().retain(|e| keep(&e.id));
    selected
        .lines_mut()
        .retain(|e| keep(&e.bus_from) && keep(&e.bus_to));
    selected
        .switches_mut()
        .retain(|e| keep(&e.bus_from) && keep(&e.bus_to));
    selected
        .transformers_mut()
        .retain(|e| e.windings.iter().all(|w| keep(&w.bus)));
    macro_rules! single {
        ($table:ident) => {
            selected.$table().retain(|e| keep(&e.bus));
        };
    }
    single!(loads_mut);
    single!(generators_mut);
    single!(ibrs_mut);
    single!(shunts_mut);
    single!(capacitors_mut);
    single!(sources_mut);
    // Location attachments must refer to equipment in the solver subset. Keep
    // the original attachments on the full portable input, including islands.
    let removed: BTreeSet<_> = islands
        .iter()
        .flat_map(|island| {
            island
                .equipment
                .iter()
                .map(|item| (item.kind.as_str(), item.name.as_str()))
        })
        .collect();
    selected.component_locations_mut().retain(|location| {
        use powerio_dist::DistLocationTarget as Target;
        let kind = match location.target {
            Target::Bus => return keep(&location.name),
            Target::Line => "line",
            Target::Switch => "switch",
            Target::Transformer => "transformer",
            Target::Load => "load",
            Target::Generator => "generator",
            Target::Ibr => "ibr",
            Target::Shunt => "shunt",
            Target::Capacitor => "capacitor",
            Target::Source => "source",
            _ => return true, // Line codes and other retained metadata.
        };
        !removed.contains(&(kind, location.name.as_str()))
    });
    selected
}

pub(crate) fn selection(
    module: &PioModule<PioValue>,
) -> Result<(Option<MulticonductorNetwork>, Vec<McExcludedIsland>), String> {
    let Some(recorded) = module.extensions().get(EXTENSION) else {
        return Ok((None, Vec::new()));
    };
    let PioValue::MulticonductorNetwork(network) = module.value() else {
        return Err(
            "energized-network selection requires a full multiconductor network".to_owned(),
        );
    };
    let recorded: Vec<McExcludedIsland> =
        serde_json::from_value(recorded.clone()).map_err(|e| e.to_string())?;
    require_source(network)?;
    let actual = mc_unpowered_islands(network)?;
    if recorded != actual {
        return Err("energized-network exclusions do not match the full input topology".to_owned());
    }
    Ok((Some(select(network, &actual)), actual))
}

/// Explicitly opt into excluding source-less bus islands. This returns the full
/// network, with a validated selection record; no equipment is erased from it.
pub fn prepare_mc_energized_network_json(text: &str) -> Result<String, String> {
    let mut module = crate::ir::deserialize_module(text)?;
    super::input::validate_mc_module(&module)?;
    if matches!(module.value(), PioValue::McAcPfInstance(_)) {
        // An existing valid instance already has source coverage on every island.
        return serde_json::to_string(&serde_json::json!({
            "module_json": text, "network_scope": null, "excluded_islands": []
        }))
        .map_err(|e| e.to_string());
    }
    let PioValue::MulticonductorNetwork(network) = module.value() else {
        return Err("energized-network preparation requires a multiconductor network".to_owned());
    };
    require_source(network)?;
    let islands = mc_unpowered_islands(network)?;
    module
        .insert_extension(
            EXTENSION,
            serde_json::to_value(&islands).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&serde_json::json!({
        "module_json": crate::ir::serialize_module(&module)?,
        "network_scope": scope(&islands), "excluded_islands": islands
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn instance(module: &PioModule<PioValue>) -> Result<powerio::McAcPfInstance, String> {
    match selection(module)?.0 {
        Some(network) => powerio::McAcPfInstance::from_network(network).map_err(|e| e.to_string()),
        None => super::input::instance_from_value(module.value()),
    }
}
