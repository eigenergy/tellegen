//! Geographic sidecar ingest, layout stamping, and `.pwd` promotion: the
//! coordinate counterpart of the balanced [`crate::ingest_case`] path.
//!
//! Parsing is powerio's [`GeoLayer`] tolerant reader (the alias table and the
//! positional branch id fallback live upstream); applying goes through
//! `Network::apply_geo_layer`, so matched bus points land in `Bus.location`
//! and matched routes in `Branch.route`, and the frontend re-reads the map
//! view from the network itself. Layers travel across the boundary as the
//! canonical `.geo.json` document (`GeoLayer::to_geojson`), which the same
//! reader accepts back.
//!
//! Every entry point is string-typed and returns `Result<_, String>` so it
//! runs in native unit tests; the `#[wasm_bindgen]` wrappers in `lib.rs` map
//! errors to `JsError` at the boundary. Input is untrusted: a malformed,
//! truncated, or oversized payload rejects as an `Err`, never a panic.

use powerio::geo::{apply_substation_points, CoordsKind, GeoApplyReport, GeoGeometry, GeoLayer};
use powerio::network::Network;
use powerio::{parse_display_bytes, DisplayData};
use tellegen::geo::{pwd_lonlat_layer, stamp_layout, Coords};

use crate::ingest_value;

/// Parse a geographic sidecar (buscoords CSV, aliased CSV/JSON records,
/// GeoJSON) from raw bytes. `hint` is the dropped file's name (picks CSV
/// against JSON; pass "" to sniff). Returns `{ layer, warnings, n_points,
/// n_routes }`: the layer as its canonical `.geo.json` document plus the
/// reader's notes on records it could not use.
pub fn parse_geo_impl(bytes: &[u8], hint: &str) -> Result<String, String> {
    let hint = (!hint.trim().is_empty()).then_some(hint);
    let parsed = GeoLayer::parse_bytes(bytes, hint).map_err(|e| e.to_string())?;
    let (mut n_points, mut n_routes) = (0usize, 0usize);
    for f in &parsed.layer.features {
        match f.geometry {
            GeoGeometry::Point(_) => n_points += 1,
            GeoGeometry::LineString(_) => n_routes += 1,
        }
    }
    serde_json::to_string(&serde_json::json!({
        "layer": parsed.layer.to_geojson(),
        "warnings": parsed.warnings,
        "n_points": n_points,
        "n_routes": n_routes,
    }))
    .map_err(|e| e.to_string())
}

/// Apply a parsed layer (the canonical `.geo.json` from [`parse_geo_impl`])
/// onto a case: matching follows uid, external id, case insensitive name, and
/// the unordered branch endpoint pair. Errors when nothing matched; otherwise
/// returns the refreshed drop-panel payload (its `network_json` now carries
/// the locations and routes) with a `report` of matched/unmatched counts.
pub fn apply_geo_impl(network_json: &str, layer_geojson: &str) -> Result<String, String> {
    let mut net = parse_network(network_json)?;
    let layer = parse_layer(layer_geojson)?;
    let report = net.apply_geo_layer(&layer);
    if report.matched_buses == 0 && report.matched_branches == 0 {
        return Err(format!(
            "no case elements matched the geographic file ({} feature(s) unmatched)",
            report.unmatched_features
        ));
    }
    payload_with_report(&net, report)
}

/// Stamp a computed layout onto a case: `coords_json` maps bus id to
/// `[lon, lat]`, `kind` is the provenance (`synthetic` for the force layout,
/// `manual` for hand placement). Returns the refreshed drop-panel payload plus
/// `layer`, the stamped layout as a canonical `.geo.json` document — ready to
/// download or to sync onto a live study.
pub fn apply_layout_impl(
    network_json: &str,
    coords_json: &str,
    kind: &str,
) -> Result<String, String> {
    let kind = match kind {
        "synthetic" => CoordsKind::Synthetic,
        "manual" => CoordsKind::Manual,
        other => {
            return Err(format!(
                "unknown layout kind '{other}'; expected synthetic or manual"
            ))
        }
    };
    let coords: Coords = serde_json::from_str(coords_json)
        .map_err(|e| format!("bad layout coordinates JSON: {e}"))?;
    let mut net = parse_network(network_json)?;
    let placed = stamp_layout(&mut net, &coords, kind);
    if placed == 0 {
        return Err("no layout bus ids matched the case".to_owned());
    }
    let layer = net
        .geo_layer()
        .extracted_geojson()
        .map_err(|e| e.to_string())?;
    let mut value = ingest_value(&net, Vec::new())?;
    let object = value
        .as_object_mut()
        .ok_or("ingest payload is not an object")?;
    object.insert("layer".to_owned(), serde_json::Value::String(layer));
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

/// Extract a case's coordinates as a canonical `.geo.json` document: one point
/// per located bus, one route per routed branch, provenance preserved. Errors
/// when the case carries no coordinates.
pub fn extract_geo_impl(network_json: &str) -> Result<String, String> {
    parse_network(network_json)?
        .geo_layer()
        .extracted_geojson()
        .map_err(|e| e.to_string())
}

/// Fill case coordinates from a PowerWorld `.pwd` display sibling: the decoded
/// substation symbols project to approximate longitude/latitude and join onto
/// buses through the `SubNum` extras key. Errors when no bus joined (the case
/// carries no substation identity, or the numbers do not line up); otherwise
/// returns the refreshed drop-panel payload with a `report`.
pub fn apply_display_geo_impl(network_json: &str, bytes: &[u8]) -> Result<String, String> {
    let display = match parse_display_bytes(bytes, "pwd").map_err(|e| e.to_string())? {
        DisplayData::PowerWorld(d) => d,
        // DisplayData is #[non_exhaustive]; PowerWorld is the only arm today.
        #[allow(unreachable_patterns)]
        _ => return Err("unsupported display format".to_owned()),
    };
    let mut net = parse_network(network_json)?;
    let layer = pwd_lonlat_layer(&display);
    let mut report = apply_substation_points(&mut net, &layer);
    if report.matched_buses == 0 {
        return Err(
            "no case buses joined the .pwd substations (no matching SubNum on the bus rows)"
                .to_owned(),
        );
    }
    report
        .notes
        .push("positions are projected from diagram coordinates and are approximate".to_owned());
    payload_with_report(&net, report)
}

fn parse_network(network_json: &str) -> Result<Network, String> {
    let mut net = Network::from_json(network_json).map_err(|e| e.to_string())?;
    // Ingested cases arrive stamped; stamping here keeps the apply surfaces
    // total for a caller holding an older payload (fills only missing uids).
    powerio_pkg::ensure_payload_uids(&mut net);
    Ok(net)
}

pub(crate) fn parse_layer(layer_geojson: &str) -> Result<GeoLayer, String> {
    GeoLayer::parse_bytes(layer_geojson.as_bytes(), Some("layer.geo.json"))
        .map(|parsed| parsed.layer)
        .map_err(|e| e.to_string())
}

/// The apply report as the JSON object every geo surface returns.
pub(crate) fn report_value(report: &GeoApplyReport) -> serde_json::Value {
    serde_json::json!({
        "matched_buses": report.matched_buses,
        "matched_branches": report.matched_branches,
        "unmatched_features": report.unmatched_features,
        "notes": report.notes,
    })
}

/// The refreshed drop-panel payload for an updated network, with the apply
/// report attached under `report`.
fn payload_with_report(net: &Network, report: GeoApplyReport) -> Result<String, String> {
    let mut value = ingest_value(net, Vec::new())?;
    let object = value
        .as_object_mut()
        .ok_or("ingest payload is not an object")?;
    object.insert("report".to_owned(), report_value(&report));
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CASE3: &str = "\
function mpc = case3geo
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;
 2 1 60 20 0 0 1 1 0 230 1 1.1 0.9;
 3 1 40 10 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 100 0 300 -300 1 100 1 250 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 0 0 0 0 1 -360 360;
 2 3 0.01 0.1 0 250 0 0 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.1 20 0;
];
";

    /// The uid-stamped `network_json` a real drop produces (the input every
    /// geo surface receives from the frontend).
    fn case3_network_json() -> String {
        let out = crate::ingest_case(CASE3, "m").expect("ingest case3");
        let v: Value = serde_json::from_str(&out).unwrap();
        v["network_json"].as_str().unwrap().to_owned()
    }

    #[test]
    fn layout_stamp_extract_parse_apply_round_trip() {
        let network_json = case3_network_json();

        // Stamp a synthetic layout onto the coordless case.
        let coords = r#"{"1": [-84.0, 33.0], "2": [-84.1, 33.1], "3": [-84.2, 33.2]}"#;
        let stamped: Value = serde_json::from_str(
            &apply_layout_impl(&network_json, coords, "synthetic").expect("apply_layout"),
        )
        .unwrap();
        assert_eq!(stamped["coords_kind"], "synthetic");
        assert_eq!(stamped["has_coords"], true);
        assert_eq!(stamped["view"]["buses"].as_array().unwrap().len(), 3);
        let stamped_json = stamped["network_json"].as_str().unwrap();
        assert!(stamped_json.contains("\"location\""));

        // The returned layer is the canonical document; extraction agrees.
        let layer = stamped["layer"].as_str().unwrap();
        assert!(layer.contains("powerio_geo"));
        assert_eq!(extract_geo_impl(stamped_json).expect("extract"), layer);

        // The layer parses back through the tolerant reader and applies onto
        // the original coordless payload, matching every bus by uid.
        let parsed: Value =
            serde_json::from_str(&parse_geo_impl(layer.as_bytes(), "case3.geo.json").unwrap())
                .unwrap();
        assert_eq!(parsed["n_points"], 3);
        let applied: Value = serde_json::from_str(
            &apply_geo_impl(&network_json, parsed["layer"].as_str().unwrap()).expect("apply_geo"),
        )
        .unwrap();
        assert_eq!(applied["report"]["matched_buses"], 3);
        assert_eq!(applied["report"]["unmatched_features"], 0);
        // Layout provenance rides through the layer: the re-applied case still
        // reads as a synthetic layout.
        assert_eq!(applied["coords_kind"], "synthetic");
    }

    #[test]
    fn buscoords_csv_places_by_external_id_and_partial_match_is_reported() {
        let network_json = case3_network_json();
        let csv = "bus_i,lat,lon\n1,33.0,-84.0\n2,33.1,-84.1\n9,40.0,-80.0\n";
        let parsed: Value =
            serde_json::from_str(&parse_geo_impl(csv.as_bytes(), "coords.csv").unwrap()).unwrap();
        let applied: Value = serde_json::from_str(
            &apply_geo_impl(&network_json, parsed["layer"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(applied["report"]["matched_buses"], 2);
        assert_eq!(applied["report"]["unmatched_features"], 1);
        // Bus 3 has no coordinates: it is omitted from the view with a warning.
        assert_eq!(applied["view"]["buses"].as_array().unwrap().len(), 2);
        assert!(applied["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("lacked coordinates")));
    }

    #[test]
    fn linestring_route_lands_in_branch_route_and_view_path() {
        let network_json = case3_network_json();
        let geojson = r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{"bus_i":1},"geometry":{"type":"Point","coordinates":[-84.0,33.0]}},
            {"type":"Feature","properties":{"bus_i":2},"geometry":{"type":"Point","coordinates":[-84.1,33.1]}},
            {"type":"Feature","properties":{"bus_i":3},"geometry":{"type":"Point","coordinates":[-84.2,33.2]}},
            {"type":"Feature","properties":{"f_bus":1,"t_bus":2},"geometry":{"type":"LineString","coordinates":[[-84.0,33.0],[-84.05,33.08],[-84.1,33.1]]}}
          ]
        }"#;
        let parsed: Value =
            serde_json::from_str(&parse_geo_impl(geojson.as_bytes(), "routes.geojson").unwrap())
                .unwrap();
        let applied: Value = serde_json::from_str(
            &apply_geo_impl(&network_json, parsed["layer"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(applied["report"]["matched_branches"], 1);
        assert!(applied["network_json"]
            .as_str()
            .unwrap()
            .contains("\"route\""));
        let branches = applied["view"]["branches"].as_array().unwrap();
        let routed = &branches[0]["path"].as_array().unwrap();
        assert_eq!(routed.len(), 3, "route polyline reaches the view");
        assert_eq!(branches[1]["path"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn geo_surfaces_reject_untrusted_input_without_panicking() {
        // Dropped files are hostile: malformed, truncated, and oversized inputs
        // must reject as an `Err` (a wasm panic aborts the instance).
        let oversized_json = format!("{{\"a\":{}", "[".repeat(50_000));
        let oversized_csv = "x,".repeat(200_000);
        let bad_bytes: &[&[u8]] = &[
            b"",
            b"   ",
            b"{",
            b"]",
            b"not a geo file",
            b"null",
            b"42",
            b"bus_i,lat,lon\n",
            b"\xff\xfe\x00\x01garbage",
            oversized_json.as_bytes(),
            oversized_csv.as_bytes(),
        ];
        for bad in bad_bytes {
            assert!(parse_geo_impl(bad, "").is_err());
            assert!(parse_geo_impl(bad, "coords.csv").is_err());
            assert!(parse_geo_impl(bad, "coords.json").is_err());
            assert!(apply_display_geo_impl(&case3_network_json(), bad).is_err());
        }

        let network_json = case3_network_json();
        let layer = {
            let parsed: Value = serde_json::from_str(
                &parse_geo_impl(b"bus_i,lat,lon\n1,33.0,-84.0\n", "c.csv").unwrap(),
            )
            .unwrap();
            parsed["layer"].as_str().unwrap().to_owned()
        };
        // Bad halves of every apply pair reject cleanly.
        for bad in ["", "{", "null", "[]", "not json"] {
            assert!(apply_geo_impl(bad, &layer).is_err());
            assert!(apply_geo_impl(&network_json, bad).is_err());
            assert!(apply_layout_impl(bad, r#"{"1":[0.0,0.0]}"#, "synthetic").is_err());
            assert!(apply_layout_impl(&network_json, bad, "synthetic").is_err());
            assert!(extract_geo_impl(bad).is_err());
        }
        // A layer whose keys match nothing errors instead of silently no-oping.
        let unmatched = {
            let parsed: Value = serde_json::from_str(
                &parse_geo_impl(b"bus_i,lat,lon\n99,33.0,-84.0\n", "c.csv").unwrap(),
            )
            .unwrap();
            parsed["layer"].as_str().unwrap().to_owned()
        };
        assert!(apply_geo_impl(&network_json, &unmatched).is_err());
        // Unknown layout kinds and unmatched layout ids fail closed.
        assert!(apply_layout_impl(&network_json, r#"{"1":[0.0,0.0]}"#, "surveyed").is_err());
        assert!(apply_layout_impl(&network_json, r#"{"99":[0.0,0.0]}"#, "manual").is_err());
        // A coordless case has nothing to extract.
        assert!(extract_geo_impl(&network_json).is_err());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn saved_study_package_carries_the_stamped_layout() {
        // The point of stamping: a study saved after a layout lands carries the
        // coordinates, so a restore places the case without re-dropping files.
        let network_json = case3_network_json();
        let stamped: Value = serde_json::from_str(
            &apply_layout_impl(
                &network_json,
                r#"{"1":[-84.0,33.0],"2":[-84.1,33.1],"3":[-84.2,33.2]}"#,
                "manual",
            )
            .unwrap(),
        )
        .unwrap();
        let mut study = tellegen::Study::new(
            stamped["network_json"].as_str().unwrap(),
            tellegen::Problem::DcOpf,
        )
        .expect("study");
        study
            .commit(
                &[tellegen::NetworkEdit::AddLoad {
                    bus: tellegen::ElementKey::Id(2),
                    p_mw: 5.0,
                }],
                tellegen::SolveOptions::default(),
            )
            .expect("commit");
        let package_json = study
            .to_package()
            .expect("to_package")
            .to_json()
            .expect("json");
        assert!(package_json.contains("\"location\""));

        // And the restore path reads them back: the bundle view is placed with
        // manual provenance.
        let bundle: Value =
            serde_json::from_str(&crate::load_package_bundle(&package_json).unwrap()).unwrap();
        assert_eq!(bundle["has_coords"], true);
        assert_eq!(bundle["coords_kind"], "manual");
        assert_eq!(bundle["view"]["buses"].as_array().unwrap().len(), 3);
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn live_study_geo_sync_lands_in_the_next_save() {
        // The frontend keeps a built Study alive across a geo apply; syncing the
        // layer through Study::apply_geo_layer makes the next save carry it.
        let network_json = case3_network_json();
        let mut study =
            tellegen::Study::new(&network_json, tellegen::Problem::DcOpf).expect("study");
        assert!(!study
            .to_package()
            .unwrap()
            .to_json()
            .unwrap()
            .contains("\"location\""));
        let layer = {
            let parsed: Value = serde_json::from_str(
                &parse_geo_impl(
                    b"bus_i,lat,lon\n1,33.0,-84.0\n2,33.1,-84.1\n3,33.2,-84.2\n",
                    "c.csv",
                )
                .unwrap(),
            )
            .unwrap();
            parsed["layer"].as_str().unwrap().to_owned()
        };
        let report = study.apply_geo_layer(&parse_layer(&layer).unwrap());
        assert_eq!(report.matched_buses, 3);
        assert!(study
            .to_package()
            .unwrap()
            .to_json()
            .unwrap()
            .contains("\"location\""));
    }
}
