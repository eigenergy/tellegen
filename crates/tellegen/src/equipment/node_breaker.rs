//! Detailed switch connectivity for drawings; never reconstruct it from reduced AC branches.
use powerio::{BalancedNetwork, ComponentId};
use powerio_tx::TopologyEndpoint;
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct NodeBreaker {
    pub stations: Vec<Station>,
    pub nodes: Vec<Node>,
    pub busbars: Vec<Busbar>,
    pub switches: Vec<Switch>,
    pub connections: Vec<Connection>,
    pub terminals: Vec<Terminal>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Station {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub voltage_level: String,
    pub station: Option<String>,
    pub bus: Option<usize>,
    pub nominal_kv: f64,
    pub container: Option<String>,
    pub container_name: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Busbar {
    pub id: String,
    pub name: String,
    pub node: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Switch {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub open: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Connection {
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Terminal {
    pub id: String,
    pub equipment: String,
    pub name: String,
    pub kind: String,
    pub number: u8,
    pub node: String,
    pub connected: bool,
}

pub fn project(net: &BalancedNetwork) -> NodeBreaker {
    let Some(d) = net.detailed_connectivity() else {
        return NodeBreaker::default();
    };
    let metadata: std::collections::HashMap<_, _> = d
        .component_metadata
        .iter()
        .map(|m| (&m.component, m))
        .collect();
    let levels: std::collections::HashMap<_, _> =
        d.voltage_levels.iter().map(|v| (&v.component, v)).collect();
    let name = |id: &ComponentId| {
        metadata
            .get(id)
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| id.local_id().to_string())
    };
    let endpoint = |e: &TopologyEndpoint| match e {
        TopologyEndpoint::Node(id) | TopologyEndpoint::Bus(id) => Some(id.to_string()),
        _ => None,
    };
    NodeBreaker {
        stations: d
            .substations
            .iter()
            .map(|s| Station {
                id: s.component.to_string(),
                name: name(&s.component),
            })
            .collect(),
        nodes: d
            .connectivity_nodes
            .iter()
            .map(|n| {
                let level = levels.get(&n.voltage_level);
                Node {
                    id: n.component.to_string(),
                    name: name(&n.component),
                    voltage_level: n.voltage_level.to_string(),
                    station: level
                        .and_then(|v| v.substation.as_ref())
                        .map(ToString::to_string),
                    bus: n.calculated_bus.map(|b| b.0),
                    nominal_kv: level.map_or(0.0, |v| v.nominal_kv),
                    container: metadata
                        .get(&n.component)
                        .and_then(|m| m.equipment_container.as_ref())
                        .map(ToString::to_string),
                    container_name: metadata
                        .get(&n.component)
                        .and_then(|m| m.equipment_container.as_ref())
                        .map(&name),
                }
            })
            .chain(d.bus_breaker_buses.iter().map(|b| {
                let level = levels.get(&b.voltage_level);
                Node {
                    id: b.component.to_string(),
                    name: name(&b.component),
                    voltage_level: b.voltage_level.to_string(),
                    station: level
                        .and_then(|v| v.substation.as_ref())
                        .map(ToString::to_string),
                    bus: b.calculated_bus.map(|id| id.0),
                    nominal_kv: level.map_or(0.0, |v| v.nominal_kv),
                    container: metadata
                        .get(&b.component)
                        .and_then(|m| m.equipment_container.as_ref())
                        .map(ToString::to_string),
                    container_name: metadata
                        .get(&b.component)
                        .and_then(|m| m.equipment_container.as_ref())
                        .map(&name),
                }
            }))
            .collect(),
        busbars: d
            .busbar_sections
            .iter()
            .map(|b| Busbar {
                id: b.component.to_string(),
                name: name(&b.component),
                node: b.node.to_string(),
            })
            .collect(),
        switches: d
            .switches
            .iter()
            .filter_map(|s| {
                Some(Switch {
                    id: s.component.to_string(),
                    name: name(&s.component),
                    kind: match s.kind {
                        powerio_tx::SwitchKind::Breaker => "breaker",
                        powerio_tx::SwitchKind::Disconnector => "disconnector",
                        powerio_tx::SwitchKind::LoadBreakSwitch => "load_break_switch",
                        _ => "switch",
                    }
                    .into(),
                    from: endpoint(&s.endpoint1)?,
                    to: endpoint(&s.endpoint2)?,
                    open: s.open,
                })
            })
            .collect(),
        connections: d
            .internal_connections
            .iter()
            .map(|c| Connection {
                from: c.node1.to_string(),
                to: c.node2.to_string(),
            })
            .collect(),
        terminals: d
            .terminals
            .iter()
            .filter_map(|t| {
                Some(Terminal {
                    id: t
                        .component
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| format!("{}:{}", t.equipment, t.terminal)),
                    equipment: t.equipment.to_string(),
                    name: name(&t.equipment),
                    kind: t.equipment.component_type().to_string(),
                    number: t.terminal,
                    node: t
                        .node
                        .as_ref()
                        .or(t.bus.as_ref())
                        .or(t.connectable_bus.as_ref())?
                        .to_string(),
                    connected: t.connected,
                })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_switch_positions_nodes_and_calculated_bus_links() {
        let module = powerio::parse(
            powerio::Source::open(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/data/node-breaker"
            ))
            .unwrap(),
        )
        .unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("balanced fixture");
        };
        let drawing = project(net);
        assert_eq!(drawing.nodes.len(), 5);
        assert_eq!(drawing.busbars.len(), 2);
        assert_eq!(drawing.switches.len(), 3);
        assert!(drawing.switches.iter().any(|s| s.name == "DSC1" && s.open));
        let n1 = drawing.nodes.iter().find(|n| n.name == "N1").unwrap();
        let n2 = drawing.nodes.iter().find(|n| n.name == "N2").unwrap();
        let n3 = drawing.nodes.iter().find(|n| n.name == "N3").unwrap();
        assert_eq!(n1.bus, n2.bus);
        assert_ne!(n1.bus, n3.bus);
        assert!(drawing
            .terminals
            .iter()
            .any(|t| t.name == "L3" && !t.connected));
    }
    #[test]
    fn mixed_topology_retains_bus_switches_and_disconnected_terminals() {
        let module = powerio::parse(
            powerio::Source::open(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/data/node-breaker"
            ))
            .unwrap(),
        )
        .unwrap();
        let powerio::PioValue::BalancedNetwork(mut net) = module.into_value() else {
            panic!("balanced fixture");
        };
        let d = std::sync::Arc::make_mut(net.detailed_connectivity_mut().as_mut().unwrap());
        let level = d.voltage_levels[0].component.clone();
        let a = ComponentId::new("bus", "configured-a").unwrap();
        let b = ComponentId::new("bus", "configured-b").unwrap();
        for (component, calculated) in [(&a, 701), (&b, 903)] {
            d.bus_breaker_buses.push(
                serde_json::from_value(serde_json::json!({
                    "component": component, "voltage_level": level, "calculated_bus": calculated
                }))
                .unwrap(),
            );
        }
        let mut switch = d.switches[0].clone();
        switch.component = ComponentId::new("switch", "bus-switch").unwrap();
        switch.endpoint1 = TopologyEndpoint::Bus(a.clone());
        switch.endpoint2 = TopologyEndpoint::Bus(b.clone());
        switch.kind = powerio_tx::SwitchKind::LoadBreakSwitch;
        switch.open = true;
        d.switches.push(switch);
        let mut terminal = d.terminals[0].clone();
        terminal.component = Some(ComponentId::new("terminal", "disconnected").unwrap());
        terminal.equipment = ComponentId::new("load", "bus-load").unwrap();
        terminal.node = None;
        terminal.bus = None;
        terminal.connectable_bus = Some(b.clone());
        terminal.connected = false;
        d.terminals.push(terminal.clone());
        terminal.component = Some(ComponentId::new("terminal", "connected").unwrap());
        terminal.bus = Some(a.clone());
        terminal.connected = true;
        d.terminals.push(terminal);
        let drawing = project(&net);
        assert_eq!(drawing.nodes.len(), 7);
        assert_eq!(
            drawing
                .nodes
                .iter()
                .find(|n| n.id == a.to_string())
                .unwrap()
                .bus,
            Some(701)
        );
        assert_eq!(
            drawing
                .nodes
                .iter()
                .find(|n| n.id == b.to_string())
                .unwrap()
                .bus,
            Some(903)
        );
        let switch = drawing
            .switches
            .iter()
            .find(|s| s.name == "bus-switch")
            .unwrap();
        assert_eq!((&switch.from, &switch.to), (&a.to_string(), &b.to_string()));
        assert_eq!(switch.kind, "load_break_switch");
        assert!(switch.open);
        let disconnected = drawing
            .terminals
            .iter()
            .find(|t| t.id == "terminal/disconnected")
            .unwrap();
        assert_eq!(disconnected.node, b.to_string());
        assert!(!disconnected.connected);
        let connected = drawing
            .terminals
            .iter()
            .find(|t| t.id == "terminal/connected")
            .unwrap();
        assert_eq!(connected.node, a.to_string());
        assert!(connected.connected);
    }
}
