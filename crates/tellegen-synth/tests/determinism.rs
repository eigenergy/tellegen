mod common;

use std::sync::Arc;

use tellegen_synth::{generate, Generator, Stage};

#[test]
fn same_request_gives_byte_identical_module_and_report() {
    let req = common::request(common::iraq_box(), 11, Some(60));
    let (m1, r1) = generate(req.clone(), common::places(), None).expect("first");
    let (m2, r2) = generate(req.clone(), common::places(), None).expect("second");
    let t1 = tellegen::ir::serialize_module(&m1).unwrap();
    let t2 = tellegen::ir::serialize_module(&m2).unwrap();
    assert_eq!(t1, t2);
    // Reports agree once timings (absent without a clock) are ignored.
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap()
    );
    assert!(t1.contains("\"location\""));
    assert!(t1.contains("\"route\""));
}

#[test]
fn the_default_request_is_byte_identical_across_runs() {
    // The automatic bus count, national demand, place snapping and the
    // adequacy areas all take part.
    let req = common::request(common::iraq_box(), 5, None);
    let (m1, r1) = generate(req.clone(), common::places(), None).expect("first");
    let (m2, r2) = generate(req, common::places(), None).expect("second");
    assert_eq!(
        tellegen::ir::serialize_module(&m1).unwrap(),
        tellegen::ir::serialize_module(&m2).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap()
    );
    let ctx = r1.context.as_ref().unwrap();
    assert!(!ctx.demand_by_country.is_empty());
    assert!(r1.provenance.demand_table.is_some());
}

#[test]
fn seed_changes_positions_and_hash_ignores_field_order() {
    let a = common::request(common::iraq_box(), 1, Some(40));
    let mut b = a.clone();
    b.seed = 2;
    let (ma, _) = generate(a.clone(), common::places(), None).unwrap();
    let (mb, _) = generate(b, common::places(), None).unwrap();
    let pos = |m: &powerio::PioModule<powerio::BalancedNetwork>| -> Vec<(f64, f64)> {
        m.value()
            .buses()
            .iter()
            .map(|b| (b.location.unwrap().x, b.location.unwrap().y))
            .collect()
    };
    assert_ne!(pos(&ma), pos(&mb));

    let json = serde_json::to_value(&a).unwrap();
    let mut reordered = serde_json::Map::new();
    let mut keys: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    keys.reverse();
    for k in keys {
        reordered.insert(k.clone(), json[&k].clone());
    }
    let text = serde_json::to_string(&serde_json::Value::Object(reordered)).unwrap();
    let back: tellegen_synth::SynthRequest = serde_json::from_str(&text).unwrap();
    assert_eq!(back.sha256(), a.sha256());

    let mut with_fallback = a.clone();
    with_fallback.fallback_from = Some(tellegen_synth::request::FallbackFrom {
        source: tellegen_synth::Source::Osm,
        reason: tellegen_synth::request::FallbackReason::OsmInsufficient,
        buses: Some(2),
        branches: Some(1),
        message: None,
    });
    assert_eq!(with_fallback.sha256(), a.sha256());
    let (_, report) = generate(with_fallback, common::places(), None).unwrap();
    assert!(report.warnings.iter().any(|w| w.code == "osm_fallback"));
    assert!(report.provenance.fallback_from.is_some());
}

#[test]
fn staged_session_matches_one_shot_and_exposes_previews() {
    let req = common::request(common::rhine_box(), 5, Some(50));
    let (one_shot, _) = generate(req.clone(), common::places(), None).unwrap();
    let mut session = Generator::new(req, common::places(), None).unwrap();
    assert_eq!(session.stage(), Stage::Context);
    assert!(session.network().is_none());
    assert!(session.module_json().is_err());
    let mut seen = Vec::new();
    let mut previews = 0;
    loop {
        let out = session.step().unwrap();
        seen.push(out.stage);
        if out.preview_changed {
            previews += 1;
            let p = session.preview();
            assert_eq!(p.buses.len(), 50);
            let arrays = session.preview_arrays();
            assert_eq!(arrays.bus_positions.len(), 100);
            assert_eq!(arrays.bus_kinds.len(), 50);
            assert_eq!(arrays.branch_start_indices.len(), p.branches.len() + 1);
            assert_eq!(arrays.branch_kv.len(), p.branches.len());
        }
        if out.stage == Stage::Electrify {
            assert!(session.network().is_some());
            assert!(session.module_json().unwrap().contains("tellegen_synth"));
        }
        assert!(out.progress > 0.0 && out.progress <= 1.0);
        if out.done {
            assert_eq!(out.next, None);
            break;
        }
    }
    assert_eq!(previews, 3);
    assert_eq!(seen[0], Stage::Context);
    assert_eq!(seen[1], Stage::SiteBuses);
    assert_eq!(seen[2], Stage::Topology);
    assert_eq!(seen[3], Stage::Electrify);
    assert_eq!(seen[4], Stage::DcOpf);
    assert!(seen.contains(&Stage::Calibrate));
    assert_eq!(seen[seen.len() - 2], Stage::AcPf);
    assert_eq!(seen[seen.len() - 1], Stage::Finalize);
    assert!(session.is_done());
    assert!(session.step().is_err());
    let (module, report) = session.into_parts().unwrap();
    assert_eq!(
        tellegen::ir::serialize_module(&module).unwrap(),
        tellegen::ir::serialize_module(&one_shot).unwrap()
    );
    assert_eq!(report.stages.len(), seen.len());
}

#[test]
fn cancellation_stops_the_next_stage() {
    let req = common::request(common::iraq_box(), 3, Some(30));
    let mut session = Generator::new(req, common::places(), None).unwrap();
    session.step().unwrap();
    session
        .cancel_flag()
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        session.step().unwrap_err(),
        tellegen_synth::SynthError::Cancelled
    );
    let _ = Arc::strong_count(&common::places());
}

#[test]
fn invalid_requests_are_rejected_before_any_stage() {
    let places = common::places();
    let mut req = common::request(common::iraq_box(), 1, Some(20));
    req.branch_factor = Some(0.9);
    assert!(Generator::new(req, places.clone(), None).is_err());
    let mut req = common::request(common::iraq_box(), 1, Some(5));
    req.generator_count = Some(6);
    assert!(Generator::new(req, places.clone(), None).is_err());
    // No levels is valid (the region's national ladder); a non-positive one
    // is not.
    let mut req = common::request(common::iraq_box(), 1, None);
    req.voltage_levels_kv = vec![500.0, -230.0];
    assert!(Generator::new(req, places.clone(), None).is_err());
    let req = common::request(
        tellegen_synth::Region {
            geometry: serde_json::json!({"type": "Point", "coordinates": [1.0, 2.0]}),
            label: "bad".into(),
        },
        1,
        None,
    );
    assert!(Generator::new(req, places.clone(), None).is_err());
    let mut req = common::request(
        tellegen_synth::Region::bbox(-180.0, -90.0, 180.0, 90.0, "world"),
        1,
        None,
    );
    req.limits.max_area_km2 = 1.0e6;
    let mut session = Generator::new(req, places.clone(), None).unwrap();
    assert!(session.step().is_err());
    let text = r#"{"region": {"geometry": {"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,0]]]}}, "unknown_field": 1}"#;
    assert!(serde_json::from_str::<tellegen_synth::SynthRequest>(text).is_err());
    let mut req = common::request(common::iraq_box(), 1, Some(10));
    req.source = tellegen_synth::Source::Osm;
    assert!(Generator::new(req, places, None).is_err());
}
