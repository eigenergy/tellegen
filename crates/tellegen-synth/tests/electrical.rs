mod common;

use powerio::BusType;
use tellegen_synth::electrical::params::{class_ratio, line_class_for};
use tellegen_synth::{generate, CalibrationStatus, Fuel, SolveOutcome, Stage};

#[test]
fn generated_network_respects_the_electrical_invariants() {
    let mut req = common::request(common::iraq_box(), 7, Some(80));
    req.voltage_levels_kv = vec![500.0, 230.0, 115.0];
    req.reserve_margin = 0.25;
    let (module, report) = generate(req.clone(), common::places(), None).expect("generate");
    let net = module.value();
    let kv_of = |id: powerio::BusId| net.buses().iter().find(|b| b.id == id).unwrap().base_kv;

    assert_eq!(net.buses().len(), 80);
    let refs = net
        .buses()
        .iter()
        .filter(|b| b.kind == BusType::Ref)
        .count();
    assert_eq!(refs, 1, "exactly one reference bus on one island");
    for br in net.branches() {
        let extras = &br.extras["tellegen_synth"];
        assert!(br.r > 0.0 && br.x > 0.0 && br.b >= 0.0, "{br:?}");
        assert!(br.rate_a > 0.0);
        let (class, ratio) = class_ratio(extras["nominal_kv"].as_f64().unwrap());
        assert_eq!(extras["class_kv"].as_f64().unwrap(), class.kv);
        let basis = extras["rating_basis"].as_str().unwrap();
        if basis == "thermal" || basis == "loadability" {
            // Never above the conductor at its own voltage, never below the
            // St. Clair floor of its surge impedance loading.
            assert!(
                br.rate_a <= class.thermal_mva * ratio + 1e-9,
                "{basis} {}",
                br.rate_a
            );
            assert!(br.rate_a >= 0.38 * class.sil_mw * ratio * ratio);
        }
        let transformer = extras["is_transformer"].as_bool().unwrap();
        let (fkv, tkv) = (kv_of(br.from), kv_of(br.to));
        if transformer {
            assert_eq!(br.tap, 1.0, "nominal per-unit ratio");
            assert!((extras["winding_ratio"].as_f64().unwrap() - fkv / tkv).abs() < 1e-9);
            assert!(fkv > tkv);
        } else {
            assert_eq!(br.tap, 0.0);
            assert!((fkv - tkv).abs() < 1e-9);
            // Line X/R stays within the class band (the spread moves r and x together).
            let class_xr = class.x_ohm_per_km / class.r_ohm_per_km;
            assert!((br.x / br.r - class_xr).abs() < 1e-6, "x/r {}", br.x / br.r);
        }
        assert!(br.route.as_ref().is_some_and(|r| r.len() >= 2));
        assert!((br.rate_b - 1.1 * br.rate_a).abs() < 1e-9);
    }
    let ctx = report.context.as_ref().unwrap();
    let load: f64 = net.loads().iter().map(|l| l.p).sum();
    assert!((load - ctx.peak_demand_mw).abs() < 1e-6 * ctx.peak_demand_mw);
    let tan_phi = req.power_factor.acos().tan();
    for l in net.loads() {
        assert!((l.q / l.p - tan_phi).abs() < 1e-9);
    }
    let cap: f64 = net.generators().iter().map(|g| g.pmax).sum();
    assert!(cap >= (1.0 + req.reserve_margin) * load - 1e-6);
    for g in net.generators() {
        let cost = g.cost.as_ref().expect("cost");
        assert_eq!(cost.model, 2);
        assert!(cost.coeffs[0] > 0.0);
        assert_eq!(g.vg, 1.02);
        assert!(g.qmax > 0.0 && g.qmin < 0.0);
    }
    let gen_buses: std::collections::HashSet<_> = net.generators().iter().map(|g| g.bus).collect();
    assert_eq!(gen_buses.len(), net.generators().len(), "one unit per bus");
    assert!(net.buses().iter().all(|b| b.location.is_some()));
    assert_eq!(report.stages.last().unwrap().stage, Stage::Finalize);
    assert!(report
        .elements
        .as_ref()
        .unwrap()
        .generators
        .iter()
        .any(|g| matches!(g.fuel, Fuel::Gas)));
    let dc = report.dc_opf.as_ref().unwrap();
    assert_eq!(dc.status, SolveOutcome::Optimal, "{:?}", dc.message);
    assert!(dc.max_loading.unwrap() <= 1.0 + 1e-6);
    let cal = report.calibration.as_ref().unwrap();
    assert!(cal.final_objective <= cal.initial_objective + 1e-12);
    if (cal.final_objective - cal.initial_objective).abs() <= 1e-12 {
        assert!(matches!(
            cal.status,
            CalibrationStatus::Unchanged | CalibrationStatus::Converged
        ));
    }
    for step in &cal.steps {
        assert_eq!(
            step.accepted,
            step.objective_after < step.objective_before - 1e-9
        );
    }
    let ac = report.ac_pf.as_ref().unwrap();
    assert_eq!(
        ac.status,
        tellegen_synth::report::AcPfOutcome::Converged,
        "{:?}",
        ac.message
    );
    assert!(
        ac.vm_min.unwrap() >= 0.9 && ac.vm_max.unwrap() <= 1.1,
        "{ac:?}"
    );
}

#[test]
fn explicit_controls_and_decimal_voltages() {
    let mut req = common::request(common::gulf_of_guinea_box(), 19, Some(24));
    req.density = tellegen_synth::Density::Medium;
    req.voltage_levels_kv = vec![330.0, 161.0, 34.5];
    req.branch_factor = Some(2.0);
    req.demand = tellegen_synth::DemandSpec::TotalMw { mw: 2400.0 };
    req.reserve_margin = 0.55;
    req.generator_count = Some(6);
    req.calibration.enabled = false;
    let (module, report) = generate(req, common::places(), None).expect("generate");
    let net = module.value();
    assert_eq!(net.buses().len(), 24);
    assert_eq!(net.generators().len(), 6);
    // The branch factor sets the target; the corridor plan exceeds it only
    // when the planning flows need more, and then says so.
    let plan = report.planning.as_ref().unwrap();
    assert_eq!(plan.branch_target, 48);
    assert_eq!(plan.branches, net.branches().len());
    assert!(net.branches().len() >= 48);
    assert_eq!(
        net.branches().len() > 48,
        report
            .warnings
            .iter()
            .any(|w| w.code == "branch_target_exceeded")
    );
    let load: f64 = net.loads().iter().map(|l| l.p).sum();
    assert!((load - 2400.0).abs() < 1e-6);
    let cap: f64 = net.generators().iter().map(|g| g.pmax).sum();
    assert!(cap >= 1.55 * 2400.0 - 1e-6);
    assert!(net.buses().iter().any(|b| (b.base_kv - 34.5).abs() < 1e-9));
    assert_eq!(line_class_for(34.5).kv, 138.0);
    let stats = report.stats.as_ref().unwrap();
    assert!(stats.parallel_branches > 0 && stats.parallel_corridors > 0);
    assert_eq!(
        stats.voltage_counts["34.5"] as usize,
        net.buses().iter().filter(|b| b.base_kv == 34.5).count()
    );
    assert_eq!(
        report.calibration.as_ref().unwrap().status,
        CalibrationStatus::Disabled
    );
    assert!(module
        .history()
        .iter()
        .any(|h| h.name() == "tellegen-synth.generate"));
    assert_eq!(module.producer().name(), "tellegen-synth");
}

#[test]
fn a_single_bus_region_still_generates() {
    let mut req = common::request(
        tellegen_synth::Region::bbox(-1.0, 6.0, -0.8, 6.2, "tiny"),
        22,
        Some(1),
    );
    req.demand = tellegen_synth::DemandSpec::TotalMw { mw: 10.0 };
    req.validation.run_ac_pf = false;
    let (module, report) = generate(req, common::places(), None).expect("generate");
    assert_eq!(module.value().buses().len(), 1);
    assert!(module.value().generators().len() <= 1);
    assert!(report.dc_opf.is_some());
}
