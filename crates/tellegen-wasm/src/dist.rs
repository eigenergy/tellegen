//! Multiconductor distribution ingest: the viewing-only counterpart of the
//! balanced [`crate::ingest_case`] path.
//!
//! A dropped OpenDSS `.dss`, PMD JSON, or BMOPF JSON parses through
//! [`powerio::parse`] into a module holding a canonical
//! [`MulticonductorNetwork`] or a multiconductor problem instance; a
//! `.pio.json` PowerIO IR document carrying a multiconductor value comes in
//! through [`tellegen::ir::deserialize_module`].
//! Either way the network is projected to a render-ready bus/terminal graph
//! ([`MulticonductorNetwork::to_graph`]) and serialized as the drop-panel payload
//! the frontend reads. No solve, no model build — this path only views
//! topology.
//!
//! The entry points ([`ingest_dist`] and [`ingest_dist_module`]) are
//! string-typed and return `Result<_, String>` so they run in native unit
//! tests; the `#[wasm_bindgen]` wrapper in `lib.rs` maps the error to a
//! `JsError` at the boundary. Input is untrusted: a malformed, truncated, or
//! oversized payload rejects as an `Err`, never a panic.

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
    let source = powerio::Source::from_memory("<case>", bytes.to_vec())
        .map_err(|e| e.to_string())?
        .with_format(format);
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    let Some(network) = multiconductor_network(module.value()) else {
        return Err(format!(
            "parsed a {} value, expected a multiconductor network or calculation",
            module.value().type_name()
        ));
    };
    ingest_dist_value(network, module.diagnostics())
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
    ingest_dist_value(network, module.diagnostics())
}

pub(crate) fn is_viewable_module_value(value: &PioValue) -> bool {
    multiconductor_network(value).is_some()
}

fn multiconductor_network(value: &PioValue) -> Option<&MulticonductorNetwork> {
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
        "graph": graph,
    }))
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
