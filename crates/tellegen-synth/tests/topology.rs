mod common;

use tellegen_synth::electrical::params::{
    class_ratio, MAX_ANGLE_SPREAD_RAD, MAX_CIRCUITS_PER_CORRIDOR, PLANNING_UTILISATION,
    STIFFENING_BRANCH_BUDGET,
};
use tellegen_synth::{generate, DemandSpec};

/// Demand in line with the ACTIVSg2000 case (67 GW on 2000 buses, about
/// 33 MW per bus), so the structure test measures the topology rather than
/// the circuits the corridor plan adds for a heavily aggregated region.
fn activsg_like_demand(n: u32) -> DemandSpec {
    DemandSpec::TotalMw {
        mw: 33.0 * n as f64,
    }
}

#[test]
fn structure_matches_reference_ranges_across_sizes() {
    for &n in &[30u32, 100, 300] {
        let mut req = common::request(common::iraq_box(), 2026, Some(n));
        req.voltage_levels_kv = vec![500.0, 230.0, 115.0];
        req.demand = activsg_like_demand(n);
        req.validation.run_ac_pf = false;
        req.validation.run_dc_opf = false;
        let (_, report) = generate(req, common::places(), None).expect("generate");
        let plan = report.planning.as_ref().unwrap();
        let stats = report.stats.as_ref().unwrap();
        // The branch factor is met exactly unless the corridor plan needed
        // more for capacity or for the angle spread, and then it says so.
        if plan.branches > plan.branch_target {
            assert!(report
                .warnings
                .iter()
                .any(|w| w.code == "branch_target_exceeded"));
            assert_eq!(plan.filler_circuits, 0);
            assert!(
                plan.capacity_circuits + plan.stiffening_circuits > 0,
                "n={n} {plan:?}"
            );
        } else {
            assert_eq!(plan.branches, plan.branch_target, "n={n} {plan:?}");
            assert!(
                (1.4..=1.5).contains(&stats.branch_bus_ratio),
                "n={n} {}",
                stats.branch_bus_ratio
            );
        }
        assert_eq!(stats.connected_components, 1, "n={n}");
        assert!(stats.algebraic_connectivity.unwrap() > 0.0, "n={n}");
        assert!(
            (2.2..=3.0).contains(&stats.avg_degree),
            "n={n} avg degree {}",
            stats.avg_degree
        );
        assert!(stats.max_degree <= 8, "n={n}");
        assert!(
            stats.delaunay_edge_fraction > 0.9,
            "n={n} {}",
            stats.delaunay_edge_fraction
        );
        assert!(stats.mst_length_ratio >= 1.0, "n={n}");
        assert!(
            stats.hv_share > 0.05 && stats.hv_share < 0.2,
            "n={n} {}",
            stats.hv_share
        );
        assert!(
            stats.reference.iter().all(|r| r.within),
            "n={n} {:?}",
            stats.reference
        );
        assert_eq!(
            stats.degree_histogram.iter().sum::<u32>() as usize,
            n as usize
        );
    }
}

#[test]
fn heavy_demand_adds_circuits_keeps_ratings_physical_and_says_so() {
    // 15 GW on 30 buses: 500 MW per bus, at stated 500/230 kV levels.
    let mut req = common::request(common::iraq_box(), 2026, Some(30));
    req.demand = DemandSpec::TotalMw { mw: 15_000.0 };
    req.voltage_levels_kv = vec![500.0, 230.0];
    req.validation.run_ac_pf = false;
    req.validation.run_dc_opf = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let net = module.value();
    let plan = report.planning.as_ref().unwrap();
    assert_eq!(plan.branches, net.branches().len());
    assert!(plan.branches > plan.branch_target, "{plan:?}");
    assert!(report
        .warnings
        .iter()
        .any(|w| w.code == "branch_target_exceeded"));
    assert!(plan.capacity_circuits + plan.stiffening_circuits > 0);
    assert!(common::check_circuit_annotations(net, &report) > 0);
    assert!(plan.converged, "{plan:?}");
    assert!(plan.max_planning_loading <= PLANNING_UTILISATION + 1e-9);
    assert!(plan.angle_spread_deg <= MAX_ANGLE_SPREAD_RAD.to_degrees() + 1e-9);
    for br in net.branches() {
        let e = &br.extras["tellegen_synth"];
        let (class, ratio) = class_ratio(e["nominal_kv"].as_f64().unwrap());
        match e["rating_basis"].as_str().unwrap() {
            "thermal" | "loadability" => assert!(br.rate_a <= class.thermal_mva * ratio + 1e-9),
            "transformer" => assert!(br.rate_a <= e["transformer_mva"].as_f64().unwrap() + 1e-9),
            other => panic!("unexpected rating basis {other}"),
        }
    }
}

#[test]
fn city_anchoring_and_parallel_circuits() {
    let mut req = common::request(common::iraq_box(), 2026, None);
    req.validation.run_ac_pf = false;
    req.validation.run_dc_opf = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let net = module.value();
    assert!(net
        .buses()
        .iter()
        .any(|b| b.name.as_deref() == Some("Baghdad")));
    assert_eq!(net.buses()[0].name.as_deref(), Some("Baghdad"));
    let elements = report.elements.as_ref().unwrap();
    assert!(elements.buses.iter().filter(|b| b.place.is_some()).count() > 5);
    let referenced: std::collections::HashSet<_> =
        net.branches().iter().flat_map(|b| [b.from, b.to]).collect();
    assert_eq!(referenced.len(), net.buses().len());
    assert_eq!(
        report.context.as_ref().unwrap().target_bus_count as usize,
        net.buses().len()
    );
}

#[test]
fn two_landmasses_within_reach_form_one_island() {
    let mut req = common::request(common::two_landmasses(), 4, Some(60));
    req.validation.run_ac_pf = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let stats = report.stats.as_ref().unwrap();
    assert_eq!(stats.connected_components, 1);
    let roles = &report.elements.as_ref().unwrap().branches;
    assert!(roles.iter().any(|b| b.boundary_role == "interconnect"));
    assert_eq!(
        module
            .value()
            .buses()
            .iter()
            .filter(|b| b.kind == powerio::BusType::Ref)
            .count(),
        1
    );
    assert_eq!(
        report.dc_opf.as_ref().unwrap().status,
        tellegen_synth::SolveOutcome::Optimal
    );
}

#[test]
fn far_islands_kept_are_validated_separately() {
    let geom = serde_json::json!({"type": "MultiPolygon", "coordinates": [
        [[[42.0, 31.0], [46.0, 31.0], [46.0, 35.0], [42.0, 35.0], [42.0, 31.0]]],
        [[[-2.0, 5.0], [1.0, 5.0], [1.0, 8.0], [-2.0, 8.0], [-2.0, 5.0]]]
    ]});
    let mut req = common::request(
        tellegen_synth::Region {
            geometry: geom,
            label: "far".into(),
        },
        4,
        Some(60),
    );
    req.islands.beyond = tellegen_synth::FarIslandPolicy::Keep;
    req.validation.run_ac_pf = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    let stats = report.stats.as_ref().unwrap();
    assert_eq!(stats.connected_components, 2);
    assert_eq!(
        module
            .value()
            .buses()
            .iter()
            .filter(|b| b.kind == powerio::BusType::Ref)
            .count(),
        2
    );
    let dc = report.dc_opf.as_ref().unwrap();
    assert_eq!(dc.islands.len(), 2);
    assert_eq!(
        dc.status,
        tellegen_synth::SolveOutcome::Optimal,
        "{:?}",
        dc.message
    );
    assert!(report.warnings.iter().any(|w| w.code == "multiple_islands"));

    let mut req = common::request(
        tellegen_synth::Region {
            geometry: serde_json::json!({"type": "MultiPolygon", "coordinates": [
                [[[42.0, 31.0], [46.0, 31.0], [46.0, 35.0], [42.0, 35.0], [42.0, 31.0]]],
                [[[-2.0, 5.0], [1.0, 5.0], [1.0, 8.0], [-2.0, 8.0], [-2.0, 5.0]]]
            ]}),
            label: "cut".into(),
        },
        4,
        Some(60),
    );
    req.validation.run_ac_pf = false;
    req.validation.run_dc_opf = false;
    let (module, report) = generate(req, common::places(), None).unwrap();
    assert_eq!(report.stats.as_ref().unwrap().connected_components, 1);
    assert!(report.warnings.iter().any(|w| w.code == "islands_cut"));
    assert!(module
        .value()
        .buses()
        .iter()
        .all(|b| b.location.unwrap().x > 40.0));
}

/// One unit feeding a 1,400 km strip at stated 500/230 kV levels, which the
/// plan may raise to 765 kV: every megawatt crosses the region.
fn strip_fed_from_one_unit(mw: f64) -> tellegen_synth::GenerationReport {
    let region = tellegen_synth::Region::bbox(30.0, 30.0, 45.0, 32.0, "strip");
    let mut req = common::request(region, 2026, Some(40));
    req.voltage_levels_kv = vec![500.0, 230.0];
    req.generator_count = Some(1);
    req.demand = DemandSpec::TotalMw { mw };
    req.validation.run_ac_pf = false;
    req.validation.run_dc_opf = false;
    generate(req, common::places(), None).unwrap().1
}

#[test]
fn a_corridor_short_of_capacity_at_the_circuit_cap_raises_its_ends_to_765_kv() {
    let report = strip_fed_from_one_unit(35_000.0);
    let plan = report.planning.as_ref().unwrap();
    assert_eq!(plan.circuit_cap, MAX_CIRCUITS_PER_CORRIDOR);
    assert!(plan.max_parallel_circuits <= plan.circuit_cap, "{plan:?}");
    assert_eq!(plan.added_level_kv, Some(765.0));
    let codes: Vec<&str> = report.warnings.iter().map(|w| w.code.as_str()).collect();
    assert!(codes.contains(&"voltage_level_added"), "{codes:?}");
    assert_eq!(report.context.as_ref().unwrap().voltage_levels_kv[0], 765.0);
    let elements = report.elements.as_ref().unwrap();
    assert!(elements
        .branches
        .iter()
        .all(|b| b.circuits <= MAX_CIRCUITS_PER_CORRIDOR));
    // What 765 kV and the cap still cannot carry is reported, not stacked.
    if plan.capped_corridors > 0 {
        assert!(codes.contains(&"planning_incomplete"), "{codes:?}");
    }
}

#[test]
fn stiffening_stops_at_the_branch_budget_and_says_the_plan_is_incomplete() {
    let report = strip_fed_from_one_unit(15_000.0);
    let plan = report.planning.as_ref().unwrap();
    let n = report.stats.as_ref().unwrap().n_buses as f64;
    assert!(!plan.converged, "{plan:?}");
    assert!(plan.angle_spread_deg > MAX_ANGLE_SPREAD_RAD.to_degrees());
    // Stiffening adds a circuit only below the budget; capacity may add more.
    let budget = (STIFFENING_BRANCH_BUDGET * n).floor() as usize;
    assert!(
        plan.branches <= budget.max(plan.branch_target) + plan.capacity_circuits,
        "{plan:?}"
    );
    let incomplete = report
        .warnings
        .iter()
        .find(|w| w.code == "planning_incomplete")
        .expect("planning_incomplete");
    assert!(
        incomplete.message.contains("angle spread"),
        "{}",
        incomplete.message
    );
}

#[test]
fn a_country_with_no_higher_level_keeps_its_ceiling_under_heavy_load() {
    // Germany's grid stops at 380 kV. Ten times its load on 60 buses needs
    // far more than two circuits per corridor, and the plan may not invent a
    // level: it stacks to the cap and says what is left.
    let mut req = common::request(
        tellegen_synth::Region::bbox(6.5, 47.6, 14.5, 54.5, "Germany box"),
        2026,
        Some(60),
    );
    req.demand = DemandSpec::TotalMw { mw: 750_000.0 };
    req.validation.run_ac_pf = false;
    req.validation.run_dc_opf = false;
    let places = std::sync::Arc::new(
        tellegen_synth::Places::parse(include_bytes!("../data/populated_places.json")).unwrap(),
    );
    let (module, report) = generate(req, places, None).unwrap();
    let ctx = report.context.as_ref().unwrap();
    assert_eq!(ctx.voltage_country.as_deref(), Some("DEU"));
    assert!(ctx.promotion_levels_kv.is_empty());
    assert_eq!(ctx.voltage_levels_kv, vec![380.0, 220.0]);
    let plan = report.planning.as_ref().unwrap();
    assert_eq!(plan.added_level_kv, None);
    assert!(module
        .value()
        .buses()
        .iter()
        .all(|b| b.base_kv <= 380.0 + 1e-9));
    let codes: Vec<&str> = report.warnings.iter().map(|w| w.code.as_str()).collect();
    assert!(!codes.contains(&"voltage_level_added"), "{codes:?}");
    assert!(plan.capped_corridors > 0, "{plan:?}");
    assert!(codes.contains(&"planning_incomplete"), "{codes:?}");
}
