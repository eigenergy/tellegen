//! Inputs a caller can send that must come back as a result or a typed
//! refusal: never a hang, an unbounded allocation or a panic.

mod common;

use std::time::{Duration, Instant};

use tellegen_synth::{generate, DemandSpec, Region, SynthError, SynthRequest};

/// One change to a request.
type Edit = Box<dyn Fn(&mut SynthRequest)>;

#[test]
fn a_zero_area_sliver_beside_a_real_polygon_does_not_stall_sizing() {
    // The sliver's bounding box spans 100 x 40 degrees. Sampling it at the
    // area sampler's finest step would visit about 5e13 grid points.
    let region = Region {
        geometry: serde_json::json!({"type": "MultiPolygon", "coordinates": [
            [[[-2.0, 5.0], [1.0, 5.0], [1.0, 8.0], [-2.0, 8.0], [-2.0, 5.0]]],
            [[[-50.0, 0.0], [50.0, 40.0], [-50.0, 0.0]]]
        ]}),
        label: "Box and sliver".into(),
    };
    let mut req = common::request(region, 1, Some(10));
    req.validation.run_ac_pf = false;
    req.calibration.enabled = false;
    let start = Instant::now();
    generate(req, common::places(), None).unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_ring_written_across_the_antimeridian_the_short_way_is_refused() {
    let fiji = Region {
        geometry: serde_json::json!({"type": "Polygon", "coordinates": [[
            [179.95, -16.9], [-179.95, -16.9], [-179.95, -16.8], [179.95, -16.8], [179.95, -16.9]
        ]]}),
        label: "Fiji, short way".into(),
    };
    let err = common::request(fiji, 1, Some(10)).validate().unwrap_err();
    assert!(err.to_string().contains("antimeridian"), "{err}");
}

#[test]
fn a_ring_around_a_pole_and_a_ring_split_at_the_seam_are_kept() {
    for geometry in [
        serde_json::json!({"type": "Polygon", "coordinates": [[
            [-180.0, -90.0], [-180.0, -70.0], [-90.0, -70.0], [0.0, -70.0], [90.0, -70.0],
            [180.0, -70.0], [180.0, -90.0], [-180.0, -90.0]
        ]]}),
        serde_json::json!({"type": "MultiPolygon", "coordinates": [
            [[[179.0, -17.0], [180.0, -17.0], [180.0, -16.0], [179.0, -16.0], [179.0, -17.0]]],
            [[[-180.0, -17.0], [-179.0, -17.0], [-179.0, -16.0], [-180.0, -16.0], [-180.0, -17.0]]]
        ]}),
    ] {
        let region = Region {
            geometry,
            label: "kept".into(),
        };
        common::request(region, 1, Some(10)).validate().unwrap();
    }
}

#[test]
fn huge_finite_values_are_refused_without_panicking() {
    let refusals: Vec<Edit> = vec![
        Box::new(|r| r.demand = DemandSpec::TotalMw { mw: 1e308 }),
        Box::new(|r| {
            r.demand = DemandSpec::Population {
                per_capita_kw: Some(1e308),
                peak_factor: 1.4,
            }
        }),
        Box::new(|r| {
            r.demand = DemandSpec::Population {
                per_capita_kw: None,
                peak_factor: 1e308,
            }
        }),
        Box::new(|r| r.voltage_levels_kv = vec![1e300]),
        Box::new(|r| r.voltage_levels_kv = vec![1e-300]),
        Box::new(|r| r.voltage_levels_kv = vec![-230.0]),
    ];
    for (i, refuse) in refusals.iter().enumerate() {
        let mut req = common::request(common::iraq_box(), 1, Some(30));
        refuse(&mut req);
        assert!(
            matches!(req.validate(), Err(SynthError::InvalidRequest(_))),
            "case {i}"
        );
    }
    let boundaries: Vec<Edit> = vec![
        Box::new(|r| r.demand = DemandSpec::TotalMw { mw: 1e7 }),
        Box::new(|r| {
            r.demand = DemandSpec::Population {
                per_capita_kw: Some(100.0),
                peak_factor: 10.0,
            }
        }),
        Box::new(|r| r.voltage_levels_kv = vec![1500.0, 1.0]),
    ];
    for (i, accept) in boundaries.iter().enumerate() {
        let mut req = common::request(common::iraq_box(), 1, Some(30));
        accept(&mut req);
        assert!(req.validate().is_ok(), "boundary {i}");
    }
}

#[test]
fn limit_caps_must_be_positive_and_finite() {
    let refusals: Vec<Edit> = vec![
        Box::new(|r| r.limits.max_area_km2 = f64::NAN),
        Box::new(|r| r.limits.max_area_km2 = f64::INFINITY),
        Box::new(|r| r.limits.max_area_km2 = 0.0),
        Box::new(|r| r.limits.max_area_km2 = -1.0),
        Box::new(|r| r.limits.max_branch_factor = f64::NAN),
        Box::new(|r| r.limits.max_branch_factor = f64::INFINITY),
        Box::new(|r| r.limits.max_branch_factor = 0.5),
        Box::new(|r| r.limits.max_branch_factor = -2.0),
    ];
    for (i, refuse) in refusals.iter().enumerate() {
        let mut req = common::request(common::iraq_box(), 1, Some(30));
        refuse(&mut req);
        assert!(
            matches!(req.validate(), Err(SynthError::InvalidRequest(_))),
            "case {i}"
        );
    }
    let mut req = common::request(common::iraq_box(), 1, Some(30));
    req.limits.max_area_km2 = 1.0;
    req.limits.max_branch_factor = 1.0;
    req.branch_factor = Some(1.0);
    assert!(req.validate().is_ok());
}

#[test]
fn unknown_keys_in_nested_options_are_refused() {
    let base = serde_json::json!({
        "region": {
            "geometry": {"type": "Polygon", "coordinates": [[[42.1, 31.1], [48.5, 31.1], [48.5, 36.8], [42.1, 36.8], [42.1, 31.1]]]},
            "label": "Iraq box"
        },
        "demand": {"kind": "total_mw", "mw": 500.0},
        "islands": {"max_interconnect_km": 100.0, "beyond": "keep"},
        "osm": {"voltage_floor_kv": 220.0},
        "calibration": {"enabled": false, "targets": tellegen_synth::CalibrationTargets::published()},
        "validation": {"run_ac_pf": false},
        "limits": {"max_bus_count": 100},
        "fallback_from": {"source": "osm", "reason": "osm_insufficient", "buses": 2}
    });
    let parsed: SynthRequest = serde_json::from_value(base.clone()).expect("every key is known");
    assert_eq!(parsed.limits.max_bus_count, 100);
    for pointer in [
        "/region",
        "/demand",
        "/islands",
        "/osm",
        "/calibration",
        "/calibration/targets",
        "/validation",
        "/limits",
        "/fallback_from",
    ] {
        let mut misspelt = base.clone();
        misspelt
            .pointer_mut(pointer)
            .and_then(|v| v.as_object_mut())
            .unwrap()
            .insert("max_buses".into(), serde_json::json!(5));
        let err = serde_json::from_value::<SynthRequest>(misspelt)
            .err()
            .unwrap_or_else(|| panic!("{pointer} accepted an unknown key"));
        assert!(
            err.to_string().contains("unknown field `max_buses`"),
            "{pointer}: {err}"
        );
    }
}
