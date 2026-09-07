//! Multiconductor case import with portable electrical data and display geometry.
//! Parsing retains the PowerIO module for subsequent calculations and exports.

use powerio::{PioModule, PioValue};
use powerio_dist::{CoordinateSpace, DistGeoMeta, DistGraphEdgeKind, MulticonductorNetwork};
use tellegen::ir::deserialize_module;

/// Parse `text` in a distribution `format` (`dss`, `bmopf`, or `pmd`) and
/// return the drop-panel payload JSON (see [`ingest_dist_value`]). The format
/// token is resolved by PowerIO's universal parser; anything else, including
/// the balanced transmission formats, is an error on this viewing path.
pub fn ingest_dist(text: &str, format: &str) -> Result<String, String> {
    ingest_dist_bytes(text.as_bytes(), format)
}

/// Byte counterpart of [`ingest_dist`]. Dropped files stay byte exact until
/// the PowerIO family reader performs its strict UTF-8 decode.
pub fn ingest_dist_bytes(bytes: &[u8], format: &str) -> Result<String, String> {
    serde_json::to_string(&ingest_dist_bytes_value(bytes, format)?).map_err(|e| e.to_string())
}

pub(crate) fn ingest_dist_bytes_value(
    bytes: &[u8],
    format: &str,
) -> Result<serde_json::Value, String> {
    let format = crate::source_format_id(format)?;
    // The angle bracketed name marks an anonymous in-memory source; the
    // readers take the case's own name (e.g. the OpenDSS circuit name).
    let raw_study_error = if matches!(format.as_str(), "dss") {
        Some(
            "OpenDSS imports are view-only for MC Study mode until finite-source and control semantics are normalized; use BMOPF or a typed MC AC PF module"
                .to_owned(),
        )
    } else if matches!(format.as_str(), "bmopf-json" | "bmopf") {
        #[cfg(feature = "mc-pf")]
        let preflight = tellegen::validate_bmopf_json(
            std::str::from_utf8(bytes).map_err(|_| "BMOPF input is not valid UTF-8".to_owned())?,
        )
        .err();
        #[cfg(not(feature = "mc-pf"))]
        let preflight = Some(
            "MC PF support is disabled in this build; distribution input remains view-only"
                .to_owned(),
        );
        preflight
    } else {
        None
    };
    let source = powerio::Source::from_memory("<case>", bytes.to_vec())
        .map_err(|e| e.to_string())?
        .with_format(format.clone());
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    let module = if let Some(reason) = raw_study_error {
        module
            .with_diagnostic(powerio::Diagnostic::new(
                powerio::DiagnosticCode::new("PARTNER.TELLEGEN.MC_PF_UNSUPPORTED")
                    .map_err(|e| e.to_string())?,
                powerio::DiagnosticSeverity::Warning,
                reason,
            ))
            .map_err(|e| e.to_string())?
    } else {
        module
    };
    let Some(network) = multiconductor_network(module.value()) else {
        return Err(format!(
            "parsed a {} value, expected a multiconductor network or calculation",
            module.value().type_name()
        ));
    };
    let payload = ingest_dist_value(network, module.diagnostics())?;
    attach_mc_study_metadata(payload, module)
}

/// Parse `text` as a `.pio.json` stored module and, when it holds a
/// multiconductor network, problem instance, or solution, return the same
/// drop-panel payload [`ingest_dist`] does. A balanced value is rejected: the
/// frontend routes those to the study and network ingest paths.
pub fn ingest_dist_module(text: &str) -> Result<String, String> {
    let module = deserialize_module(text)?;
    serde_json::to_string(&ingest_dist_module_value(module)?).map_err(|e| e.to_string())
}

/// Byte counterpart of [`ingest_dist_module`]; the module reader's strict
/// UTF-8 and lineage checks apply.
pub fn ingest_dist_module_bytes(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "stored module document is not valid UTF-8".to_owned())?;
    ingest_dist_module(text)
}

pub(crate) fn ingest_dist_module_value(
    module: PioModule<PioValue>,
) -> Result<serde_json::Value, String> {
    let Some(network) = multiconductor_network(module.value()) else {
        return Err(format!(
            "stored module holds {}, expected a multiconductor network or calculation",
            module.value().type_name()
        ));
    };
    let payload = ingest_dist_value(network, module.diagnostics())?;
    attach_mc_study_metadata(payload, module)
}

/// Retain the electrical module and report whether its physics can be calculated.
/// Unsupported source data stays recorded in diagnostics across IR reloads.
fn attach_mc_study_metadata(
    payload: serde_json::Value,
    module: PioModule<PioValue>,
) -> Result<serde_json::Value, String> {
    let mut payload = crate::with_module_json(payload, module)?;
    #[cfg(feature = "mc-pf")]
    let reason = tellegen::validate_mc_module_json(
        payload["module_json"]
            .as_str()
            .ok_or("missing multiconductor module")?,
    )
    .err();
    #[cfg(not(feature = "mc-pf"))]
    let reason = Some("Multiconductor AC power flow is disabled in this build".to_owned());
    payload["mc_pf_supported"] = reason.is_none().into();
    payload["mc_pf_reason"] = serde_json::to_value(&reason).map_err(|e| e.to_string())?;
    payload["mc_pf_unavailable_reason"] = payload["mc_pf_reason"].clone();
    Ok(payload)
}

pub(crate) fn is_viewable_module_value(value: &PioValue) -> bool {
    multiconductor_network(value).is_some()
}

pub(crate) fn multiconductor_network(value: &PioValue) -> Option<&MulticonductorNetwork> {
    match value {
        PioValue::MulticonductorNetwork(network) => Some(network),
        PioValue::McAcPfInstance(instance) => Some(instance.network()),
        PioValue::McAcOpfInstance(instance) => Some(instance.network()),
        PioValue::McAcPfSolution(solution) => Some(solution.network()),
        PioValue::McAcOpfSolution(solution) => Some(solution.network()),
        _ => None,
    }
}

/// Everything the drop panel needs from one multiconductor parse: the case
/// name and element counts, total connected load and generation (kW),
/// structured PowerIO diagnostics, coordinate provenance, and the full
/// bus/terminal graph the frontend renders. `graph` is the serde
/// form of [`powerio_dist::DistGraph`]: buses carry their terminals, grounded
/// terminals, optional `xy`, and terminal attachments; edges carry their
/// kind, endpoints, per-conductor terminal pairs, and open/closed state.
fn ingest_dist_value(
    net: &MulticonductorNetwork,
    diagnostics: &[powerio::Diagnostic],
) -> Result<serde_json::Value, String> {
    let graph = net.to_graph();
    let graph_value = graph_with_neutral_metadata(net, &graph)?;

    let mut n_line = 0usize;
    let mut n_switch = 0usize;
    let mut n_transformer = 0usize;
    for edge in &graph.edges {
        match edge.kind {
            DistGraphEdgeKind::Line => n_line += 1,
            DistGraphEdgeKind::Switch => n_switch += 1,
            DistGraphEdgeKind::Transformer => n_transformer += 1,
            // `DistGraphEdgeKind` is non-exhaustive; a future kind still counts
            // in `n_edge` but not in any of the three named tallies.
            _ => {}
        }
    }

    let load_kw: f64 = graph.buses.iter().map(|b| b.load_kw).sum();
    let gen_kw: f64 = graph.buses.iter().map(|b| b.gen_kw).sum();
    let placed = graph.buses.iter().filter(|b| b.xy.is_some()).count();
    let has_coords = placed > 0;

    let coords_space = coords_space(net.geo().as_ref());
    let coords_kind = coords_kind(coords_space, has_coords);

    Ok(serde_json::json!({
        "name": net.name(),
        // Discriminates this payload from the balanced `ingest_case` shape so the
        // frontend routes it to the viewing-only multiconductor state.
        "model": "multiconductor",
        "mc_pf_enabled": cfg!(feature = "mc-pf"),
        "mc_pf_unavailable_reason": diagnostics.iter()
            .find(|d| d.code() == "PARTNER.TELLEGEN.MC_PF_UNSUPPORTED")
            .map(|d| d.message()),
        "geo_layer": powerio::dist_geo::to_dist_geo_layer(net).to_geojson(),
        "n_bus": graph.buses.len(),
        "n_edge": graph.edges.len(),
        "n_line": n_line,
        "n_switch": n_switch,
        "n_transformer": n_transformer,
        "n_load": net.loads().len(),
        "n_generator": net.generators().len(),
        "n_ibr": net.ibrs().len(),
        "n_source": net.sources().len(),
        "n_shunt": net.shunts().len(),
        // Only the BMOPF reader gives a capacitor its own type. A `.dss` or PMD
        // capacitor reads as a shunt, so the two counts do not overlap.
        "n_capacitor": net.capacitors().len(),
        "load_kw": load_kw,
        "gen_kw": gen_kw,
        "base_frequency": net.base_frequency(),
        "has_coords": has_coords,
        // How many buses carry a position; the rest fall back to the layout.
        "placed_buses": placed,
        "coords_space": coords_space,
        "coords_kind": coords_kind,
        "diagnostics": diagnostics,
        "graph": graph_value,
    }))
}

/// Add the neutral identity used by the MC Study view to PowerIO's stable
/// graph projection.  PowerIO deliberately keeps terminal-role conventions in
/// network extras, so this small presentation field must be derived here. An
/// explicit bus or BMOPF network convention wins; the familiar `n`, `4`, and
/// `neutral` aliases are only a fallback when no convention is declared.
fn graph_with_neutral_metadata(
    net: &MulticonductorNetwork,
    graph: &powerio_dist::DistGraph,
) -> Result<serde_json::Value, String> {
    let buses_by_id: std::collections::BTreeMap<_, _> = net
        .buses()
        .iter()
        .map(|bus| (bus.id.to_ascii_lowercase(), bus))
        .collect();
    let mut value = serde_json::to_value(graph).map_err(|e| e.to_string())?;
    let Some(buses) = value
        .get_mut("buses")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Err("PowerIO graph omitted its bus array".to_owned());
    };
    for graph_bus in buses {
        let Some(id) = graph_bus.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(bus) = buses_by_id.get(&id.to_ascii_lowercase()) else {
            continue;
        };
        if let Some(neutral) = neutral_terminal(net, bus) {
            graph_bus
                .as_object_mut()
                .expect("serialized graph bus is an object")
                .insert(
                    "neutral_terminal".to_owned(),
                    neutral.map_or(serde_json::Value::Null, serde_json::Value::String),
                );
        }
    }
    Ok(value)
}

fn neutral_terminal(
    net: &MulticonductorNetwork,
    bus: &powerio_dist::DistBus,
) -> Option<Option<String>> {
    if let Some(value) = bus.extras.get("neutral_terminal") {
        return Some(
            value
                .as_str()
                .filter(|name| bus.terminals.iter().any(|t| t == *name))
                .map(str::to_owned),
        );
    }
    if let Some(value) = bus.extras.get("neutral") {
        return Some(
            value
                .as_str()
                .filter(|name| bus.terminals.iter().any(|t| t == *name))
                .map(str::to_owned),
        );
    }
    if let Some(value) = bus.extras.get("terminal_conventions") {
        if declared_neutral_present(value) {
            return Some(declared_neutral_name(value, &bus.terminals));
        }
    }

    let global = net
        .extras()
        .get("bmopf_terminal_conventions")
        .or_else(|| net.extras().get("terminal_conventions"));
    if let Some(value) = global {
        if declared_neutral_present(value) {
            return Some(declared_neutral_name(value, &bus.terminals));
        }
    }

    let aliases = ["n", "4", "neutral"];
    let mut matches = bus.terminals.iter().filter(|terminal| {
        aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(terminal.trim()))
    });
    let first = matches.next()?.clone();
    if matches.next().is_some() {
        Some(None)
    } else {
        Some(Some(first))
    }
}

fn declared_neutral_name(value: &serde_json::Value, terminals: &[String]) -> Option<String> {
    let names = value
        .get("neutral")
        .and_then(|neutral| neutral.as_array())?;
    let matches: Vec<_> = names
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|name| terminals.iter().any(|terminal| terminal == *name))
        .collect();
    (matches.len() == 1).then(|| matches[0].to_owned())
}

fn declared_neutral_present(value: &serde_json::Value) -> bool {
    value.get("neutral").is_some()
}

/// The network's declared coordinate space as a stable snake-case token. `none`
/// when the network declared no space at all.
fn coords_space(geo: Option<&DistGeoMeta>) -> &'static str {
    match geo.map(|g| &g.space) {
        Some(CoordinateSpace::Geographic { .. }) => "geographic",
        Some(CoordinateSpace::Projected { .. }) => "projected",
        Some(CoordinateSpace::Diagram { .. }) => "diagram",
        Some(CoordinateSpace::Unknown) => "unknown",
        None => "none",
        // `CoordinateSpace` is non-exhaustive; treat any future space as
        // planar (not directly mappable), which `coords_kind` handles.
        Some(_) => "unknown",
    }
}

/// The frontend placement hint. `geographic` positions are longitude/latitude
/// and drop straight onto the map; `planar` positions carry a shape but no
/// earth referent, so the frontend fits them into a box at a placement center;
/// `synthetic` has no usable positions and falls back to the force layout.
fn coords_kind(space: &str, has_coords: bool) -> &'static str {
    if !has_coords {
        return "synthetic";
    }
    if space == "geographic" {
        "geographic"
    } else {
        "planar"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// A three-bus three-phase feeder: source, a line to a load bus, and a
    /// transformer to a secondary. No coordinates, so the frontend lays it out
    /// synthetically.
    const MICRO_DSS: &str = include_str!("../tests/fixtures/dist/micro.dss");

    /// A minimal BMOPF network with geographic bus coordinates, one line, one
    /// load, and a voltage source.
    const MICRO_BMOPF: &str = include_str!("../tests/fixtures/dist/micro_bmopf.json");

    /// A minimal PMD ENGINEERING document (one bus), recognized by its
    /// `data_model` marker.
    const MICRO_PMD: &str = include_str!("../tests/fixtures/dist/micro_pmd.json");

    fn parse(out: &str) -> Value {
        serde_json::from_str(out).expect("ingest output is JSON")
    }

    #[cfg(feature = "mc-pf")]
    const PF_BMOPF: &str =
        include_str!("../../tellegen/tests/data/mc_pf/oracle_inputs/pf_1ph_line.json");

    #[test]
    #[cfg(feature = "mc-pf")]
    fn imported_case_solves_after_geographic_attachment_and_ir_reload() {
        let payload = parse(&ingest_dist(PF_BMOPF, "bmopf-json").unwrap());
        let module = payload["module_json"].as_str().unwrap();
        let options = tellegen::McPfOptions::default();
        let before: tellegen::McPfResult =
            serde_json::from_str(&tellegen::solve_mc_module_json(module, &options).unwrap())
                .unwrap();
        let layer = serde_json::json!({
            "type": "FeatureCollection", "features": [
                {"type": "Feature", "properties": {"bus": "src"},
                 "geometry": {"type": "Point", "coordinates": [-83.9, 35.9]}},
                {"type": "Feature", "properties": {"bus": "lb"},
                 "geometry": {"type": "Point", "coordinates": [-83.8, 35.8]}},
                {"type": "Feature", "properties": {"bus_from": "src", "bus_to": "lb"},
                 "geometry": {"type": "LineString", "coordinates": [[-83.9, 35.9], [-83.85, 35.87], [-83.8, 35.8]]}}
            ]
        });
        let updated = parse(&crate::geo::apply_geo_impl(module, &layer.to_string()).unwrap());
        assert_eq!(updated["report"]["matched_buses"], 2);
        assert_eq!(updated["report"]["matched_branches"], 1);
        assert_eq!(updated["coords_kind"], "geographic");
        let saved = updated["module_json"].as_str().unwrap();
        let reloaded = parse(&ingest_dist_module(saved).unwrap());
        assert_eq!(reloaded["graph"], updated["graph"]);
        assert_eq!(reloaded["geo_layer"], updated["geo_layer"]);
        let after: tellegen::McPfResult =
            serde_json::from_str(&tellegen::solve_mc_module_json(saved, &options).unwrap())
                .unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(after).unwrap()
        );
        assert!(crate::geo::extract_geo_impl(saved)
            .unwrap()
            .contains("-83.85"));
    }

    #[test]
    #[cfg(feature = "mc-pf")]
    fn unsupported_source_physics_remains_explicit_after_ir_reload() {
        let mut raw: Value = serde_json::from_str(PF_BMOPF).unwrap();
        raw["voltage_source"]["source"]["r1"] = serde_json::json!(0.1);
        let payload = parse(&ingest_dist(&raw.to_string(), "bmopf-json").unwrap());
        assert!(payload["mc_pf_unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("finite impedance"));
        let saved = payload["module_json"].as_str().unwrap();
        assert!(
            tellegen::solve_mc_module_json(saved, &tellegen::McPfOptions::default())
                .unwrap_err()
                .contains("MC_PF_UNSUPPORTED")
        );
        assert_eq!(
            parse(&ingest_dist_module(saved).unwrap())["mc_pf_unavailable_reason"],
            payload["mc_pf_unavailable_reason"]
        );
    }

    #[test]
    fn dss_feeder_reports_counts_graph_and_synthetic_coords() {
        let v = parse(&ingest_dist(MICRO_DSS, "dss").expect("ingest micro dss"));

        assert_eq!(v["model"], "multiconductor");
        assert_eq!(v["name"], "micro");
        // sourcebus, loadbus, secondary.
        assert_eq!(v["n_bus"].as_u64().unwrap(), 3);
        assert_eq!(v["n_line"].as_u64().unwrap(), 1);
        assert_eq!(v["n_transformer"].as_u64().unwrap(), 1);
        assert_eq!(v["n_load"].as_u64().unwrap(), 2);
        assert_eq!(v["n_source"].as_u64().unwrap(), 1);
        // 500 kW + 120 kW of connected load.
        assert!((v["load_kw"].as_f64().unwrap() - 620.0).abs() < 1.0);

        // No coordinates in the file, so the frontend lays it out.
        assert!(!v["has_coords"].as_bool().unwrap());
        assert_eq!(v["coords_kind"], "synthetic");

        // The graph carries buses with terminals and edges with endpoints.
        let buses = v["graph"]["buses"].as_array().unwrap();
        assert_eq!(buses.len(), 3);
        assert!(buses
            .iter()
            .all(|b| !b["terminals"].as_array().unwrap().is_empty()));
        let edges = v["graph"]["edges"].as_array().unwrap();
        assert!(edges.iter().any(|e| e["kind"] == "line"));
        assert!(edges.iter().any(|e| e["kind"] == "transformer"));
        // Every edge names two buses that exist in the graph.
        let ids: Vec<&str> = buses.iter().map(|b| b["id"].as_str().unwrap()).collect();
        for e in edges {
            assert!(ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(e["from"].as_str().unwrap())));
            assert!(ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(e["to"].as_str().unwrap())));
        }
        #[cfg(feature = "mc-pf")]
        {
            assert_eq!(v["mc_pf_supported"], false);
            assert!(tellegen::validate_mc_module_json(v["module_json"].as_str().unwrap()).is_err());
            assert!(v["mc_pf_reason"].as_str().unwrap().contains("view-only"));
        }
    }

    #[test]
    fn bmopf_reports_geographic_coords_and_terminal_attachments() {
        let v = parse(&ingest_dist(MICRO_BMOPF, "bmopf").expect("ingest micro bmopf"));

        assert_eq!(v["model"], "multiconductor");
        assert_eq!(v["name"], "micro-bmopf");
        assert_eq!(v["n_bus"].as_u64().unwrap(), 2);
        assert_eq!(v["n_line"].as_u64().unwrap(), 1);
        assert_eq!(v["n_source"].as_u64().unwrap(), 1);
        assert_eq!(v["n_load"].as_u64().unwrap(), 1);

        // Geographic longitude/latitude ride straight onto the map.
        assert!(v["has_coords"].as_bool().unwrap());
        assert_eq!(v["coords_space"], "geographic");
        assert_eq!(v["coords_kind"], "geographic");
        assert_eq!(v["placed_buses"].as_u64().unwrap(), 2);

        let buses = v["graph"]["buses"].as_array().unwrap();
        let src = buses
            .iter()
            .find(|b| b["id"].as_str().unwrap().eq_ignore_ascii_case("src"))
            .expect("src bus");
        // xy is [x, y] = [lon, lat].
        let xy = src["xy"].as_array().unwrap();
        assert!((xy[0].as_f64().unwrap() - (-83.92)).abs() < 1e-6);
        assert!((xy[1].as_f64().unwrap() - 35.96).abs() < 1e-6);

        // The source and the load badge their bus at their terminals.
        let load_bus = buses
            .iter()
            .find(|b| b["id"].as_str().unwrap() == "load_bus")
            .expect("load bus");
        let attachments = &load_bus["terminal_attachments"];
        let kinds: Vec<&str> = attachments
            .as_object()
            .unwrap()
            .values()
            .flat_map(|v| v.as_array().unwrap())
            .map(|a| a["kind"].as_str().unwrap())
            .collect();
        assert!(
            kinds.contains(&"load"),
            "load attachment present: {kinds:?}"
        );
        #[cfg(feature = "mc-pf")]
        {
            assert_eq!(v["mc_pf_supported"], true);
            assert!(!v["module_json"].as_str().unwrap().is_empty());
            assert!(v["mc_pf_reason"].is_null());
        }
    }

    #[test]
    fn bmopf_preserves_explicit_custom_neutral_terminal_in_each_bus_graph_node() {
        let mut raw: Value = serde_json::from_str(MICRO_BMOPF).expect("fixture JSON");
        raw["terminal_conventions"] = serde_json::json!({
            "phase": ["1", "2", "3"],
            "neutral": ["return"],
            "earth": []
        });
        for bus in raw["bus"].as_object_mut().unwrap().values_mut() {
            let terminals = bus["terminal_names"].as_array_mut().unwrap();
            terminals[3] = Value::String("return".to_owned());
            bus["perfectly_grounded_terminals"] = serde_json::json!(["return"]);
        }
        let value = parse(&ingest_dist(&raw.to_string(), "bmopf").expect("custom neutral"));
        for bus in value["graph"]["buses"].as_array().unwrap() {
            assert_eq!(bus["neutral_terminal"], "return");
        }
    }

    #[cfg(feature = "mc-pf")]
    #[test]
    fn bmopf_preflight_failure_retains_module_with_unsupported_diagnostic() {
        let mut raw: Value = serde_json::from_str(MICRO_BMOPF).expect("fixture JSON");
        raw["load"]["ld1"]["model"] = Value::String("mystery_model".to_owned());
        let v = parse(&ingest_dist(&raw.to_string(), "bmopf").expect("view malformed load"));
        assert_eq!(v["model"], "multiconductor");
        assert_eq!(v["mc_pf_supported"], false);
        assert!(tellegen::validate_mc_module_json(v["module_json"].as_str().unwrap()).is_err());
        assert!(v["mc_pf_reason"].as_str().unwrap().contains("model"));
    }

    #[test]
    fn pmd_data_model_marker_is_accepted() {
        // A PMD ENGINEERING document is recognized by its `data_model` marker.
        // A tiny one with a single bus still parses to a graph.
        let v = parse(&ingest_dist(MICRO_PMD, "pmd").expect("ingest pmd"));
        assert_eq!(v["model"], "multiconductor");
        assert_eq!(v["n_bus"].as_u64().unwrap(), 1);
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(ingest_dist("", "matpower").is_err());
        assert!(ingest_dist(MICRO_DSS, "m").is_err());
    }

    #[test]
    fn format_tokens_keep_case_and_historical_alias_normalization() {
        assert!(ingest_dist(MICRO_DSS, "DSS").is_ok());
        assert!(ingest_dist(MICRO_BMOPF, "BMOPF_JSON").is_ok());
    }

    #[test]
    fn malformed_and_oversized_input_rejects_without_panicking() {
        // Every one of these must return an `Err` (a wasm panic aborts the
        // instance) rather than crashing the parser.
        let big_open = "{".repeat(50_000);
        let big_array = format!("[{}]", "0,".repeat(100_000));
        let deep = "[".repeat(20_000);
        let jagged = r#"{"bus": {"b1": {"terminal_names": 42}}}"#;
        let bmopf_cases = [
            "",
            "   ",
            "{",
            "]",
            "not json at all",
            "null",
            "[]",
            "42",
            "{\"bus\":",
            jagged,
            big_open.as_str(),
            big_array.as_str(),
            deep.as_str(),
        ];
        for bad in bmopf_cases {
            // BMOPF and PMD are the JSON readers; a bad document must not panic.
            let _ = ingest_dist(bad, "bmopf");
            let _ = ingest_dist(bad, "pmd");
        }
        // The BMOPF reader is liberal (unknown JSON lands in extras/untyped), so a
        // structurally valid but empty object still parses; the guarantee under
        // test is only that none of the above panics. Truncated JSON must error.
        assert!(ingest_dist("{", "bmopf").is_err());
        assert!(ingest_dist("]", "bmopf").is_err());
        assert!(ingest_dist(&deep, "bmopf").is_err());
    }

    #[test]
    fn oversized_dss_input_rejects_without_panicking() {
        // The DSS reader tolerates junk (unknown commands warn), so the contract
        // is no panic, not necessarily an error. A giant blob must terminate.
        let big = "New Line.".repeat(200_000);
        let _ = ingest_dist(&big, "dss");
        let _ = ingest_dist(&"~".repeat(500_000), "dss");
        let _ = ingest_dist("Clear\nSolve\n", "dss");
    }

    #[test]
    fn balanced_module_is_rejected_by_the_multiconductor_path() {
        // A stored module holding a balanced network must not be viewed as
        // multiconductor; the frontend routes it to the network ingest and
        // study-restore paths instead.
        let net = powerio::BalancedNetwork::in_memory("demo", 100.0, vec![], vec![]);
        let module = powerio::PioModule::new(powerio::PioValue::BalancedNetwork(net));
        let json = tellegen::ir::serialize_module(&module).expect("module json");
        let error = ingest_dist_module(&json).expect_err("balanced module rejected");
        assert!(error.contains("powerio.BalancedNetwork"), "{error}");
    }

    #[test]
    fn module_ingest_rejects_untrusted_input_without_panicking() {
        let big = "\"".repeat(100_000);
        for bad in ["", "   ", "{", "null", "[]", "42", "not json", big.as_str()] {
            assert!(ingest_dist_module(bad).is_err());
        }
    }
}
