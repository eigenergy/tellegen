//! Synthetic grid generation in the browser: the [`tellegen_synth`] session
//! behind string-typed bodies, so every entry point runs in native unit tests
//! and the `#[wasm_bindgen]` wrappers in `lib.rs` only map errors.
//!
//! The populated places table is loaded once per wasm instance and shared by
//! every session. Errors cross the boundary as the generator's
//! [`ErrorPayload`] JSON (`{code, message, buses?, branches?}`), so the
//! caller can tell an Overpass document too thin to build from
//! (`osm_insufficient`) from a malformed request without parsing prose.

use std::cell::RefCell;
use std::sync::Arc;

use powerio::PioValue;
use tellegen_synth::{ErrorPayload, Generator, Places, SynthError, SynthRequest};

use crate::{ingest_value, with_module_json};

thread_local! {
    /// The places table every session of this instance reads.
    static PLACES: RefCell<Option<Arc<Places>>> = const { RefCell::new(None) };
}

/// The error as the JSON payload the caller switches on.
fn payload(error: &SynthError) -> String {
    serde_json::to_string(&ErrorPayload::from(error)).unwrap_or_else(|_| error.to_string())
}

/// A failure outside the generator (serialization, the ingest view), carried
/// in the same payload shape.
fn other(code: &str, message: impl std::fmt::Display) -> String {
    serde_json::to_string(&ErrorPayload {
        code: code.to_owned(),
        message: message.to_string(),
        buses: None,
        branches: None,
    })
    .unwrap_or_else(|_| message.to_string())
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| other("build", e))
}

fn parse_request(request_json: &str) -> Result<SynthRequest, String> {
    serde_json::from_str(request_json)
        .map_err(|e| payload(&SynthError::InvalidRequest(format!("request JSON: {e}"))))
}

/// Parse and keep the places table; returns `{count, sha256}`.
pub fn load_places_impl(bytes: &[u8]) -> Result<String, String> {
    let places = Places::parse(bytes).map_err(|e| payload(&e))?;
    let summary = serde_json::json!({ "count": places.len(), "sha256": places.sha256() });
    PLACES.with(|slot| *slot.borrow_mut() = Some(Arc::new(places)));
    to_json(&summary)
}

/// The Overpass QL queries for a request's region, as a JSON array of strings.
pub fn overpass_queries_impl(request_json: &str) -> Result<String, String> {
    let request = parse_request(request_json)?;
    let queries = tellegen_synth::osm::overpass_queries(&request).map_err(|e| payload(&e))?;
    to_json(&queries)
}

/// Open a session on the loaded places table.
pub fn session_new_impl(request_json: &str, osm_json: Option<String>) -> Result<Generator, String> {
    let request = parse_request(request_json)?;
    let places = PLACES.with(|slot| slot.borrow().clone()).ok_or_else(|| {
        payload(&SynthError::Places(
            "no places table is loaded; call synth_load_places first".to_owned(),
        ))
    })?;
    Generator::new(request, places, osm_json).map_err(|e| payload(&e))
}

/// Run one stage; returns the `StepOutcome` JSON.
pub fn advance_impl(session: &mut Generator) -> Result<String, String> {
    let outcome = session.step().map_err(|e| payload(&e))?;
    to_json(&outcome)
}

/// The map preview as records.
pub fn preview_impl(session: &Generator) -> Result<String, String> {
    to_json(&session.preview())
}

/// The report so far (complete after `Done`).
pub fn report_impl(session: &Generator) -> Result<String, String> {
    to_json(session.report())
}

/// The module as generation 2 PowerIO IR text, from `Electrify` on.
pub fn module_impl(session: &Generator) -> Result<String, String> {
    session.module_json().map_err(|e| payload(&e))
}

/// The drop-panel payload a dropped `.pio.json` of this module would produce
/// (`ingest_case` shape, with `module_json`), so the opener needs no second
/// parse.
pub fn ingest_impl(session: &Generator) -> Result<String, String> {
    let module = session.module().map_err(|e| payload(&e))?;
    let view = ingest_value(module.value(), module.diagnostics(), Vec::new(), None)
        .map_err(|e| other("build", e))?;
    let value = with_module_json(view, module.map_value(PioValue::BalancedNetwork))
        .map_err(|e| other("build", e))?;
    to_json(&value)
}

/// The network as GridFM graphkit JSON, from `Electrify` on.
pub fn export_gridfm_impl(session: &Generator) -> Result<String, String> {
    session.export_gridfm().map_err(|e| payload(&e))
}

/// Every stage in one call: `{module_json, report, ingest}`.
pub fn generate_impl(request_json: &str, osm_json: Option<String>) -> Result<String, String> {
    let mut session = session_new_impl(request_json, osm_json)?;
    while !session.is_done() {
        session.step().map_err(|e| payload(&e))?;
    }
    let ingest: serde_json::Value =
        serde_json::from_str(&ingest_impl(&session)?).map_err(|e| other("build", e))?;
    to_json(&serde_json::json!({
        "module_json": module_impl(&session)?,
        "report": session.report(),
        "ingest": ingest,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACES_FIXTURE: &[u8] =
        include_bytes!("../../tellegen-synth/tests/fixtures/places_small.json");

    fn request(buses: u32) -> String {
        serde_json::json!({
            "region": {
                "geometry": {"type": "Polygon", "coordinates": [[[-2.0, 5.0], [1.0, 5.0], [1.0, 8.0], [-2.0, 8.0], [-2.0, 5.0]]]},
                "label": "Gulf of Guinea"
            },
            "target_bus_count": buses
        })
        .to_string()
    }

    #[test]
    fn a_session_runs_every_stage_and_ingests_as_a_located_module() {
        let loaded: serde_json::Value =
            serde_json::from_str(&load_places_impl(PLACES_FIXTURE).unwrap()).unwrap();
        assert_eq!(loaded["count"], 145);
        let mut session = session_new_impl(&request(40), None).unwrap();
        let mut stages = Vec::new();
        loop {
            let outcome: serde_json::Value =
                serde_json::from_str(&advance_impl(&mut session).unwrap()).unwrap();
            stages.push(outcome["stage"].as_str().unwrap().to_owned());
            if outcome["stage"] == "electrify" {
                // The module and the ingest view exist from here on.
                assert!(module_impl(&session).is_ok());
            }
            if outcome["done"] == true {
                break;
            }
        }
        assert_eq!(stages.first().map(String::as_str), Some("context"));
        assert_eq!(stages.last().map(String::as_str), Some("finalize"));
        assert!(stages.iter().any(|s| s == "calibrate"));

        let preview: serde_json::Value =
            serde_json::from_str(&preview_impl(&session).unwrap()).unwrap();
        assert_eq!(preview["buses"].as_array().unwrap().len(), 40);

        let ingest: serde_json::Value =
            serde_json::from_str(&ingest_impl(&session).unwrap()).unwrap();
        assert_eq!(ingest["has_coords"], true);
        assert_eq!(ingest["coords_kind"], "synthetic");
        assert_eq!(ingest["coords_method"], "generated");
        assert_eq!(crate::SYNTH_HISTORY_NAME, tellegen_synth::HISTORY_NAME);
        assert_eq!(ingest["n_bus"], 40);
        let branches = ingest["view"]["branches"].as_array().unwrap();
        assert!(!branches.is_empty());
        assert!(branches
            .iter()
            .all(|b| b["path"].as_array().unwrap().len() >= 2));

        // The module reads back through the drop route as a located module.
        let module = module_impl(&session).unwrap();
        let drop = crate::ingest_json_drop_value(module.as_bytes()).unwrap();
        assert_eq!(drop.kind, "module");
        assert_eq!(drop.payload["coords_kind"], "synthetic");
        assert_eq!(drop.payload["coords_method"], "generated");

        let gridfm: serde_json::Value =
            serde_json::from_str(&export_gridfm_impl(&session).unwrap()).unwrap();
        assert_eq!(gridfm["n_bus"], 40);

        let report: serde_json::Value =
            serde_json::from_str(&report_impl(&session).unwrap()).unwrap();
        assert_eq!(report["dc_opf"]["status"], "optimal");
        assert_eq!(report["ac_pf"]["status"], "converged");
        assert!(report["planning"]["branches"].as_u64().unwrap() > 0);
    }

    #[test]
    fn the_generated_module_opens_as_a_study_and_solves() {
        load_places_impl(PLACES_FIXTURE).unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&generate_impl(&request(30), None).unwrap()).unwrap();
        let module_json = out["module_json"].as_str().unwrap();
        let study = tellegen::Study::new(module_json, tellegen::Problem::DcOpf).unwrap();
        let objective = study.solution().objective.unwrap();
        let reported = out["report"]["dc_opf"]["objective"].as_f64().unwrap();
        assert!((objective - reported).abs() <= 1e-6 * reported.abs().max(1.0));
        assert_eq!(out["ingest"]["n_bus"], 30);
    }

    #[test]
    fn an_overpass_document_builds_a_mapped_grid_and_thin_data_says_so() {
        load_places_impl(PLACES_FIXTURE).unwrap();
        let request = serde_json::json!({
            "region": {"geometry": {"type": "Polygon", "coordinates": [[[8.0, 49.6], [9.5, 49.6], [9.5, 50.6], [8.0, 50.6], [8.0, 49.6]]]}},
            "source": "osm",
            "demand": {"kind": "total_mw", "mw": 1500.0}
        })
        .to_string();
        let osm = include_str!("../../tellegen-synth/tests/fixtures/overpass_small.json");
        let out: serde_json::Value =
            serde_json::from_str(&generate_impl(&request, Some(osm.to_owned())).unwrap()).unwrap();
        assert_eq!(out["ingest"]["has_coords"], true);
        assert_eq!(out["ingest"]["coords_kind"], "file");
        assert!(out["ingest"].get("coords_method").is_none());
        assert_eq!(out["report"]["dc_opf"]["status"], "optimal");
        assert_eq!(
            out["report"]["provenance"]["osm"]["counts"]["hvdc_links"],
            1
        );

        let thin = r#"{"elements":[{"type":"node","id":1,"lat":50.0,"lon":8.2,"tags":{"power":"substation","voltage":"110000"}}]}"#;
        let e = generate_impl(&request, Some(thin.to_owned()))
            .err()
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&e).unwrap();
        assert_eq!(payload["code"], "osm_insufficient");
        assert_eq!(payload["buses"], 1);
    }

    #[test]
    fn errors_carry_their_code() {
        let err = session_new_impl(&request(10), None);
        // No table loaded on this test thread yet.
        if let Err(e) = err {
            let payload: serde_json::Value = serde_json::from_str(&e).unwrap();
            assert_eq!(payload["code"], "places");
        }
        load_places_impl(PLACES_FIXTURE).unwrap();
        let e = session_new_impl(r#"{"region": 3}"#, None).err().unwrap();
        let payload: serde_json::Value = serde_json::from_str(&e).unwrap();
        assert_eq!(payload["code"], "invalid_request");
        let e = session_new_impl(&request(0), None).err().unwrap();
        let payload: serde_json::Value = serde_json::from_str(&e).unwrap();
        assert_eq!(payload["code"], "invalid_request");
        let queries: Vec<String> =
            serde_json::from_str(&overpass_queries_impl(&request(10)).unwrap()).unwrap();
        assert_eq!(queries.len(), 1);
    }
}
