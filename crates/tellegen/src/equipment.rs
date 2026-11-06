//! Source equipment for transmission displays, shared by HTTP and WASM.
//!
//! These are input settings, not results or a promise that a control is solved.
//! Rows remain in source order; HVDC never enters the AC branch index space.
use powerio::{BalancedNetwork, TransformerControlMode};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Equipment {
    pub transformers: Vec<Transformer>,
    pub shunts: Vec<Shunt>,
    pub hvdc: Vec<Hvdc>,
    pub three_winding: Vec<ThreeWinding>,
    pub devices: Vec<Device>,
    /// Detailed records may overlap reduced HVDC links. Count, do not duplicate
    /// or infer a multi-terminal graph without an authoritative mapping.
    pub detailed_dc: DetailedDc,
    pub dc_topology: DcTopology,
    pub dc_model: Option<crate::dc_grid::DcGrid>,
    pub dc_model_unavailable: Option<String>,
    pub import_notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DcTopology {
    pub nodes: Vec<String>,
    pub links: Vec<DcLink>,
    pub converters: Vec<DcConverterView>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DcLink {
    pub id: String,
    pub kind: &'static str,
    pub from: Option<String>,
    pub to: Option<String>,
    pub connected: Option<bool>,
    pub resistance_ohm: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DcConverterView {
    pub id: String,
    pub kind: &'static str,
    pub terminal1: Option<String>,
    pub terminal2: Option<String>,
    pub connected1: Option<bool>,
    pub connected2: Option<bool>,
    pub ac_bus: Option<usize>,
    pub control: Option<powerio_tx::AcDcConverterControlMode>,
}
fn dc_topology(net: &BalancedNetwork) -> DcTopology {
    let Some(d) = net.detailed_connectivity() else {
        return DcTopology::default();
    };
    let terminal = |t: &powerio_tx::DcTerminal| {
        t.dc_node
            .as_ref()
            .or(t.dc_topological_node.as_ref())
            .map(ToString::to_string)
    };
    let connected =
        |a: &powerio_tx::DcTerminal, b: &powerio_tx::DcTerminal| match (a.connected, b.connected) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        };
    let mut links: Vec<_> = d
        .dc_lines
        .iter()
        .map(|l| DcLink {
            id: l.component.to_string(),
            kind: "line",
            from: terminal(&l.dc_terminal1),
            to: terminal(&l.dc_terminal2),
            connected: connected(&l.dc_terminal1, &l.dc_terminal2),
            resistance_ohm: l.resistance_ohm,
        })
        .collect();
    links.extend(d.dc_switches.iter().map(|l| DcLink {
        id: l.component.to_string(),
        kind: "switch",
        from: terminal(&l.dc_terminal1),
        to: terminal(&l.dc_terminal2),
        connected: if l.open == Some(true) {
            Some(false)
        } else if l.open == Some(false) {
            connected(&l.dc_terminal1, &l.dc_terminal2)
        } else {
            None
        },
        resistance_ohm: l.resistance_ohm,
    }));
    links.extend(d.dc_series_devices.iter().map(|l| DcLink {
        id: l.component.to_string(),
        kind: "series",
        from: terminal(&l.dc_terminal1),
        to: terminal(&l.dc_terminal2),
        connected: connected(&l.dc_terminal1, &l.dc_terminal2),
        resistance_ohm: l.resistance_ohm,
    }));
    links.extend(d.dc_grounds.iter().map(|l| DcLink {
        id: l.component.to_string(),
        kind: "ground",
        from: terminal(&l.dc_terminal),
        to: None,
        connected: l.dc_terminal.connected,
        resistance_ohm: l.resistance_ohm,
    }));
    let converters: Vec<_> = d
        .voltage_source_converters
        .iter()
        .map(|c| DcConverterView {
            id: c.component.to_string(),
            kind: "vsc",
            terminal1: terminal(&c.dc_terminal1),
            terminal2: terminal(&c.dc_terminal2),
            connected1: c.dc_terminal1.connected,
            connected2: c.dc_terminal2.connected,
            ac_bus: c
                .pcc_terminal
                .as_ref()
                .and_then(|t| crate::dc_grid::ac_terminal_bus(net, t)),
            control: c.control_mode,
        })
        .chain(d.line_commutated_converters.iter().map(|c| {
            DcConverterView {
                id: c.component.to_string(),
                kind: "lcc",
                terminal1: terminal(&c.dc_terminal1),
                terminal2: terminal(&c.dc_terminal2),
                connected1: c.dc_terminal1.connected,
                connected2: c.dc_terminal2.connected,
                ac_bus: c
                    .pcc_terminal
                    .as_ref()
                    .and_then(|t| crate::dc_grid::ac_terminal_bus(net, t)),
                control: c.control_mode,
            }
        }))
        .collect();
    let mut nodes: std::collections::BTreeSet<_> =
        d.dc_nodes.iter().map(|n| n.component.to_string()).collect();
    // A terminal may name a topological node directly. Preserve that identity
    // rather than inventing a physical-node mapping.
    for link in &links {
        nodes.extend(link.from.iter().chain(link.to.iter()).cloned());
    }
    for c in &converters {
        nodes.extend(c.terminal1.iter().chain(c.terminal2.iter()).cloned());
    }
    DcTopology {
        nodes: nodes.into_iter().collect(),
        links,
        converters,
    }
}

/// Physical three-terminal device and its analysis provenance.
#[derive(Clone, Debug, Serialize)]
pub struct ThreeWinding {
    pub id: usize,
    pub uid: Option<String>,
    pub name: Option<String>,
    pub buses: [usize; 3],
    pub in_service: bool,
    pub star_bus: Option<usize>,
    pub branches: Vec<usize>,
}

/// Additional families whose identity must survive even where solving is unsupported.
#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub key: String,
    pub uid: Option<String>,
    pub kind: &'static str,
    pub buses: Vec<usize>,
    pub in_service: bool,
    pub treatment: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DetailedDc {
    pub nodes: usize,
    pub lines: usize,
    pub converters: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Transformer {
    pub branch: usize,
    pub uid: Option<String>,
    pub phase_shifter: bool,
    pub tap_changers: Vec<powerio_tx::TapChanger>,
    pub tap_ratio: f64,
    pub phase_shift_deg: f64,
    pub control_mode: Option<TransformerControlMode>,
    pub control_enabled: Option<bool>,
    pub controlled_bus: Option<usize>,
    /// Degrees for phase control, ratio for voltage/reactive control; absent
    /// for controls whose source limits have no defined units.
    pub setting_min: Option<f64>,
    pub setting_max: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Shunt {
    pub id: usize,
    pub uid: Option<String>,
    pub bus: usize,
    pub in_service: bool,
    /// At 1 pu voltage, in native power units. Positive B supplies vars.
    pub g_mw: f64,
    pub b_mvar: f64,
    pub section_count: Option<u32>,
    pub control_mode: Option<powerio::SwitchedShuntMode>,
    pub blocks: Vec<ShuntBlock>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShuntBlock {
    pub steps: u32,
    pub g_mw: f64,
    pub b_mvar: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hvdc {
    pub id: usize,
    pub uid: Option<String>,
    pub from: usize,
    pub to: usize,
    pub in_service: bool,
    /// Withdrawal from the from bus; injection into the to bus. Signed input
    /// values, including reverse transfers, not newly calculated DC results.
    pub pf_mw: f64,
    pub pt_mw: f64,
    pub nominal_kv: Option<f64>,
    pub from_converter: Option<powerio_tx::HvdcConverterKind>,
    pub to_converter: Option<powerio_tx::HvdcConverterKind>,
}

/// Project source rows before analysis lowering/filtering. Normalize only the
/// displayed units, not identity, state or the number of physical devices.
pub fn project(net: &BalancedNetwork) -> Result<Equipment, String> {
    let normalized = net.is_normalized();
    let power = if normalized {
        net.check_base_mva().map_err(|e| e.to_string())?;
        net.base_mva()
    } else {
        1.0
    };
    let angle = if normalized {
        180.0 / std::f64::consts::PI
    } else {
        1.0
    };
    let mut taps_by_branch: std::collections::HashMap<&str, Vec<powerio_tx::TapChanger>> =
        std::collections::HashMap::new();
    if let Some(d) = net.detailed_connectivity() {
        for tap in &d.tap_changers {
            if tap.transformer.component_type() == "branch" {
                taps_by_branch
                    .entry(tap.transformer.local_id())
                    .or_default()
                    .push(tap.clone());
            }
        }
    }
    let transformers = net
        .branches()
        .iter()
        .enumerate()
        .filter_map(|(row, br)| {
            let tap_changers = br
                .uid
                .as_deref()
                .and_then(|uid| taps_by_branch.get(uid))
                .cloned()
                .unwrap_or_default();
            // Normalization replaces a line's zero tap with unity. Unity alone then
            // cannot establish physical transformer identity.
            let transformer = if normalized {
                br.tap != 1.0 || br.shift != 0.0
            } else {
                br.is_transformer()
            };
            if !transformer && br.control.is_none() && tap_changers.is_empty() {
                return None;
            }
            let mode = br.control.as_ref().map(|c| c.mode);
            let phase_shifter = tap_changers
                .iter()
                .any(|t| t.kind == powerio_tx::TapChangerKind::Phase)
                || matches!(
                    mode,
                    Some(
                        TransformerControlMode::ActiveFlow
                            | TransformerControlMode::AsymmetricActiveFlow
                    )
                );
            let setting_scale = match mode {
                Some(
                    TransformerControlMode::ActiveFlow
                    | TransformerControlMode::AsymmetricActiveFlow,
                ) => Some(angle),
                Some(TransformerControlMode::Voltage | TransformerControlMode::ReactiveFlow) => {
                    Some(1.0)
                }
                _ => None,
            };
            Some(Transformer {
                branch: row + 1,
                uid: br.uid.clone(),
                phase_shifter,
                tap_changers,
                tap_ratio: if br.tap == 0.0 { 1.0 } else { br.tap },
                phase_shift_deg: br.shift * angle,
                control_mode: mode,
                control_enabled: br.control.as_ref().map(|c| c.enabled),
                controlled_bus: br
                    .control
                    .as_ref()
                    .and_then(|c| c.controlled_bus.map(|b| b.0)),
                setting_min: br
                    .control
                    .as_ref()
                    .and_then(|c| setting_scale.map(|s| c.tap_min * s)),
                setting_max: br
                    .control
                    .as_ref()
                    .and_then(|c| setting_scale.map(|s| c.tap_max * s)),
            })
        })
        .collect();
    let shunts = net
        .shunts()
        .iter()
        .enumerate()
        .map(|(row, s)| Shunt {
            id: row + 1,
            uid: s.uid.clone(),
            bus: s.bus.0,
            in_service: s.in_service,
            g_mw: s.g * power,
            b_mvar: s.b * power,
            section_count: s.section_count,
            control_mode: s.control.as_ref().map(|c| c.mode),
            blocks: s
                .control
                .as_ref()
                .map(|c| {
                    c.blocks
                        .iter()
                        .map(|b| ShuntBlock {
                            steps: b.steps,
                            g_mw: b.g * power,
                            b_mvar: b.b * power,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    let hvdc = net
        .hvdc()
        .iter()
        .enumerate()
        .map(|(row, h)| Hvdc {
            id: row + 1,
            uid: h.uid.clone(),
            from: h.from.0,
            to: h.to.0,
            in_service: h.in_service,
            pf_mw: h.pf * power,
            pt_mw: h.pt * power,
            nominal_kv: h.nominal_voltage_kv,
            from_converter: h.converter1.as_ref().map(|c| c.kind),
            to_converter: h.converter2.as_ref().map(|c| c.kind),
        })
        .collect();
    let analysis = powerio_tx::IndexedNetwork::new(net);
    let mut expanded = 0;
    let three_winding = net
        .transformers_3w()
        .iter()
        .enumerate()
        .map(|(row, t)| {
            let (star_bus, branches) = if t.in_service {
                let bus = analysis.network().buses()[net.buses().len() + expanded]
                    .id
                    .0;
                let first = net.branches().len() + expanded * 3 + 1;
                expanded += 1;
                (Some(bus), (first..first + 3).collect())
            } else {
                (None, Vec::new())
            };
            ThreeWinding {
                id: row + 1,
                uid: t.uid.clone(),
                name: t.name.clone(),
                buses: t.windings.each_ref().map(|w| w.bus.0),
                in_service: t.in_service,
                star_bus,
                branches,
            }
        })
        .collect();
    let devices = net
        .static_var_compensators()
        .iter()
        .enumerate()
        .map(|(row, d)| Device {
            key: format!("svc:{}", row + 1),
            uid: d.uid.clone(),
            kind: "svc",
            buses: vec![d.bus.0],
            in_service: d.in_service,
            treatment: "unsupported",
        })
        .chain(net.storage().iter().enumerate().map(|(row, d)| Device {
            key: format!("storage:{}", row + 1),
            uid: d.uid.clone(),
            kind: "storage",
            buses: vec![d.bus.0],
            in_service: d.in_service,
            treatment: "unsupported",
        }))
        .chain(net.switches().iter().enumerate().map(|(row, d)| Device {
            key: format!("switch:{}", row + 1),
            uid: d.uid.clone(),
            kind: "switch",
            buses: vec![d.from.0, d.to.0],
            in_service: d.closed,
            treatment: if d.closed { "unsupported" } else { "open" },
        }))
        .collect();
    let detailed_dc = net
        .detailed_connectivity()
        .as_ref()
        .map(|d| DetailedDc {
            nodes: d.dc_nodes.len(),
            lines: d.dc_lines.len(),
            converters: d.voltage_source_converters.len() + d.line_commutated_converters.len(),
        })
        .unwrap_or_default();
    let dc_topology = dc_topology(net);
    let (dc_model, dc_model_unavailable) = if dc_topology.nodes.is_empty() {
        (None, None)
    } else {
        match crate::dc_grid::from_network(net) {
            Ok(grid) => (Some(grid), None),
            Err(reason) => (None, Some(reason)),
        }
    };
    Ok(Equipment {
        transformers,
        shunts,
        hvdc,
        three_winding,
        devices,
        detailed_dc,
        dc_topology,
        dc_model,
        dc_model_unavailable,
        import_notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio::{
        Branch, Bus, BusId, Hvdc as SourceHvdc, Shunt as SourceShunt, TransformerControl,
    };

    fn fixture() -> BalancedNetwork {
        let mut net = BalancedNetwork::new("equipment", 100.0);
        net.buses_mut().extend([
            Bus::new(BusId(1), powerio::BusType::Ref, 138.0),
            Bus::new(BusId(2), powerio::BusType::Pq, 138.0),
        ]);
        net.generators_mut().push(powerio::Generator::new(BusId(1)));
        let mut pst = Branch::new(BusId(1), BusId(2), 0.0, 0.1);
        pst.x = 0.1;
        pst.tap = 1.0;
        let mut control = TransformerControl::new(TransformerControlMode::ActiveFlow);
        control.tap_min = -20.0;
        control.tap_max = 20.0;
        pst.control = Some(control);
        let mut fixed = pst.clone();
        fixed.control = None;
        fixed.shift = 30.0;
        net.branches_mut().extend([pst, fixed]);
        net.shunts_mut().extend([
            SourceShunt::new(BusId(1), 0.0, 25.0),
            SourceShunt::new(BusId(1), 0.0, -25.0),
        ]);
        let mut link = SourceHvdc::new(BusId(1), BusId(2));
        link.pf = -90.0;
        link.pt = -95.0;
        net.hvdc_mut().push(link);
        net
    }

    #[test]
    fn source_identity_and_zero_angle_pst_survive_projection() {
        let mut net = fixture();
        net.shunts_mut()[1].in_service = false;
        let e = project(&net).unwrap();
        assert!(e.transformers[0].phase_shifter);
        assert_eq!(e.transformers[0].phase_shift_deg, 0.0);
        assert!(!e.transformers[1].phase_shifter);
        assert_eq!(e.shunts.len(), 2);
        assert!(!e.shunts[1].in_service);
        assert_eq!(e.shunts[1].id, 2);
        assert_eq!(e.hvdc[0].pf_mw - e.hvdc[0].pt_mw, 5.0);
    }

    #[test]
    fn three_winding_mapping_keeps_inactive_source_rows() {
        let mut net = fixture();
        net.buses_mut()
            .push(Bus::new(BusId(3), powerio::BusType::Pq, 138.0));
        let t = powerio::Transformer3W::new(
            [1, 2, 3].map(|i| powerio::Winding::new(BusId(i))),
            [powerio::Impedance::new(0.01, 0.1, 100.0); 3],
        );
        let mut off = t.clone();
        off.in_service = false;
        net.transformers_3w_mut().extend([off, t]);
        let e = project(&net).unwrap();
        assert_eq!(e.three_winding[0].star_bus, None);
        assert_eq!(e.three_winding[1].star_bus, Some(4));
        assert_eq!(e.three_winding[1].branches, vec![3, 4, 5]);
        assert_eq!(e.three_winding[1].id, 2);
    }
    #[test]
    fn active_svc_is_visible_and_rejected_instead_of_omitted() {
        let mut net = fixture();
        net.static_var_compensators_mut()
            .push(powerio_tx::StaticVarCompensator::new(BusId(2), -0.1, 0.1));
        assert_eq!(project(&net).unwrap().devices[0].kind, "svc");
        let error = crate::api::solve_network(&net, &Default::default()).unwrap_err();
        assert!(error.contains("static VAR"), "{error}");
    }

    #[test]
    fn normalized_projection_restores_native_units() {
        let raw = fixture();
        let norm = raw.to_normalized().unwrap();
        let a = project(&raw).unwrap();
        let b = project(&norm).unwrap();
        assert!(
            (a.transformers[1].phase_shift_deg - b.transformers[1].phase_shift_deg).abs() < 1e-10
        );
        assert!((b.transformers[0].setting_min.unwrap() + 20.0).abs() < 1e-10);
        assert_eq!(a.shunts[0].b_mvar, b.shunts[0].b_mvar);
        assert_eq!(a.hvdc[0].pf_mw, b.hvdc[0].pf_mw);
        assert_eq!(a.hvdc[0].pt_mw, b.hvdc[0].pt_mw);
    }
}
