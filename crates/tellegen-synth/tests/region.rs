mod common;

use tellegen_synth::geom::region::RegionShape;
use tellegen_synth::{generate, Region};

#[test]
fn polygon_regions_keep_buses_inside_and_tag_tie_lines() {
    let triangle = Region {
        geometry: serde_json::json!({"type": "Polygon", "coordinates": [[[42.0, 31.0], [48.0, 31.0], [42.0, 37.0], [42.0, 31.0]]]}),
        label: "Triangle".into(),
    };
    let shape = RegionShape::from_geometry(&triangle.geometry).unwrap();
    let mut req = common::request(triangle, 42, Some(40));
    req.validation.run_ac_pf = false;
    req.calibration.enabled = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let net = module.value();
    assert!(net
        .buses()
        .iter()
        .all(|b| shape.contains(b.location.unwrap().x, b.location.unwrap().y)));
    let roles: Vec<&str> = report
        .elements
        .as_ref()
        .unwrap()
        .branches
        .iter()
        .map(|b| b.boundary_role.as_str())
        .collect();
    assert!(roles.iter().all(|r| *r == "internal" || *r == "tie_line"));
    assert!(report.warnings.iter().all(|w| w.code != "islands_cut"));
}

#[test]
fn holes_hold_no_buses() {
    let geom = serde_json::json!({"type": "Polygon", "coordinates": [
        [[42.0, 31.0], [48.0, 31.0], [48.0, 37.0], [42.0, 37.0], [42.0, 31.0]],
        [[44.0, 33.0], [46.0, 33.0], [46.0, 35.0], [44.0, 35.0], [44.0, 33.0]]
    ]});
    let mut req = common::request(
        Region {
            geometry: geom,
            label: "Holed".into(),
        },
        8,
        Some(80),
    );
    req.validation.run_ac_pf = false;
    req.calibration.enabled = false;
    let (module, _) = generate(req, common::places(), None).unwrap();
    for b in module.value().buses() {
        let l = b.location.unwrap();
        assert!(
            !((44.0..=46.0).contains(&l.x) && (33.0..=35.0).contains(&l.y)),
            "{l:?}"
        );
    }
}

#[test]
fn antimeridian_region_wraps_output_and_stays_narrow() {
    let geom = serde_json::json!({"type": "MultiPolygon", "coordinates": [
        [[[176.0, -19.0], [180.0, -19.0], [180.0, -16.0], [176.0, -16.0], [176.0, -19.0]]],
        [[[-180.0, -19.0], [-178.0, -19.0], [-178.0, -16.0], [-180.0, -16.0], [-180.0, -19.0]]]
    ]});
    let shape = RegionShape::from_geometry(&geom).unwrap();
    assert!(shape.frame.east > 180.0);
    let mut req = common::request(
        Region {
            geometry: geom,
            label: "Seam".into(),
        },
        5,
        Some(30),
    );
    req.validation.run_ac_pf = false;
    req.calibration.enabled = false;
    req.islands.max_interconnect_km = 400.0;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let net = module.value();
    let lons: Vec<f64> = net.buses().iter().map(|b| b.location.unwrap().x).collect();
    assert!(lons.iter().all(|l| (-180.0..=180.0).contains(l)));
    assert!(lons.iter().any(|l| *l > 170.0) && lons.iter().any(|l| *l < -170.0));
    assert!(report.context.as_ref().unwrap().crosses_antimeridian);
    assert!(net
        .buses()
        .iter()
        .any(|b| b.name.as_deref() == Some("Labasa")));
    for br in net.branches() {
        for p in br.route.as_ref().unwrap() {
            assert!((-180.0..=180.0).contains(&p.x));
        }
    }
}

#[test]
fn bbox_helper_splits_at_the_seam() {
    let r = Region::bbox(170.0, -20.0, 190.0, -15.0, "x");
    assert_eq!(r.geometry["type"], "MultiPolygon");
    let shape = RegionShape::from_geometry(&r.geometry).unwrap();
    assert!((shape.frame.east - 190.0).abs() < 1e-9);
    assert!((shape.frame.west - 170.0).abs() < 1e-9);
}

#[test]
fn no_city_region_falls_back_to_blue_noise() {
    let mut req = common::request(
        Region::bbox(-30.0, -50.0, -10.0, -30.0, "South Atlantic"),
        3,
        Some(40),
    );
    req.validation.run_ac_pf = false;
    req.calibration.enabled = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    assert!(module.value().buses().iter().all(|b| b
        .name
        .as_deref()
        .unwrap()
        .starts_with("Substation ")));
    assert!(report
        .warnings
        .iter()
        .any(|w| w.code == "no_population_signal"));
    assert_eq!(report.stats.as_ref().unwrap().connected_components, 1);
}
