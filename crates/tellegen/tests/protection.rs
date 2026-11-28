//! Protection relays on RMS runs: settings by rule, the setting audit, the
//! standard test suite and the static fault study.
//!
//! WECC 240 ships with the code. The IEEE 14-bus cases are case data
//! (`tests/data/rms_examples/README.md`); their tests skip when the data is
//! not on this machine.
#![cfg(feature = "protection")]

use std::path::PathBuf;

use tellegen::protection::setting::{
    audit, auto_scheme, carry_ratings, estimate_ratings, AutoSetting, SettingOptions, Severity,
};
use tellegen::protection::testing::{run_suite, standard_suite, SuiteOptions, SuiteReport};
use tellegen::protection::OperationKind;
use tellegen::rms::{LoadModel, RmsEvent, RmsRequest, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/case_data.rs"
));

/// The IEEE 14-bus validation case of the case data (`rms_validation/ieee14`).
fn ieee14() -> Option<PathBuf> {
    let found = std::env::var_os("TELLEGEN_CASE_DATA")
        .map(|d| PathBuf::from(d).join("rms_validation/ieee14"))
        .filter(|d| d.join("case.raw").is_file());
    if found.is_none() {
        assert!(
            std::env::var_os("TELLEGEN_REQUIRE_CASE_DATA").is_none(),
            "case data `rms_validation/ieee14` is required (TELLEGEN_REQUIRE_CASE_DATA) but not found"
        );
        eprintln!("skipped: case data `rms_validation/ieee14` is not on this machine");
    }
    found
}

fn load(dir: &std::path::Path) -> (powerio::BalancedNetwork, powerio::DynamicCase) {
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    (net.clone(), dyr)
}

/// Relays by rule as the app sets them: branches the case leaves unrated get
/// the typical rating of their voltage level, at least 125 % of their flow
/// at the operating point, and their relays carry it.
fn set_by_rule(sim: &RmsSimulation) -> AutoSetting {
    let mut topo = sim.topology().clone();
    let estimated = estimate_ratings(&mut topo, &sim.branch_loading());
    let mut set = auto_scheme(&topo, &SettingOptions::default());
    carry_ratings(&mut set.scheme, &topo, &estimated);
    set
}

fn print_failures(report: &SuiteReport) {
    for o in report.tests.iter().filter(|o| !o.passed) {
        println!("FAIL {} {:?}", o.name, o.error);
        for c in &o.checks {
            println!(
                "   [{}] {} — {}",
                if c.passed { "ok" } else { "XX" },
                c.expectation,
                c.observed
            );
        }
    }
}

/// Every relay set by rule on WECC 240 passes the audit without errors, and
/// nothing picks up at the operating point.
#[test]
fn wecc240_setting_audit_and_operating_point() {
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let t = std::time::Instant::now();
    let set = set_by_rule(&sim);
    let t_set = t.elapsed().as_secs_f64();
    let mut rated = sim.topology().clone();
    estimate_ratings(&mut rated, &sim.branch_loading());
    let findings = audit(&rated, &set.scheme);
    let errors: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    let tests = standard_suite(sim.topology(), &set.scheme, &SuiteOptions::default());
    println!(
        "WECC 240: {} relays set in {t_set:.2} s, {} findings, {} standard tests",
        set.scheme.relays.len(),
        findings.len(),
        tests.len()
    );
    assert!(!set.scheme.relays.is_empty());
    assert!(errors.is_empty(), "audit errors: {errors:?}");
    // WECC 240's power flow holds a few buses above 1.1 pu: there the
    // overvoltage (59, 60 s) and overexcitation (24) stages set at 1.1 pu
    // pick up, which the operating point is there to show. Nothing else.
    let op = sim.relays_at_operating_point(&set.scheme).unwrap();
    for p in &op.pickups {
        println!("operating point: {} {} {}", p.relay, p.function, p.detail);
    }
    assert!(
        op.pickups
            .iter()
            .all(|p| (p.function == "24" || p.function == "59 U>") && p.detail.contains(" 1.1")),
        "only overvoltage at buses above 1.1 pu picks up: {:?}",
        op.pickups
    );
}

/// The standard suite on WECC 240, its distance tests limited to a few lines.
#[test]
#[ignore = "about a minute in release; run with --include-ignored"]
fn wecc240_suite() {
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let set = set_by_rule(&sim);
    let opts = SuiteOptions {
        max_lines: Some(4),
        ..SuiteOptions::default()
    };
    let report = run_suite(&sim, &set.scheme, &opts);
    println!(
        "{} tests: {} passed, {} failed",
        report.tests.len(),
        report.passed,
        report.failed
    );
    print_failures(&report);
    assert_eq!(report.failed, 0);
}

/// A transient fault in the middle of a WECC 240 line: both line ends trip
/// in zone 1 and the auto-recloser closes them again.
#[test]
fn wecc240_line_fault_trips_both_ends_and_recloses() {
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let set = set_by_rule(&sim);
    let topo = sim.topology();
    // The first line (not a transformer) with distance relays at both ends.
    let line = topo
        .branches
        .iter()
        .find(|b| {
            !b.transformer
                && set
                    .scheme
                    .relays
                    .iter()
                    .any(|r| r.id == format!("L{}-{}", b.from_number, b.to_number))
                && set
                    .scheme
                    .relays
                    .iter()
                    .any(|r| r.id == format!("L{}-{}", b.to_number, b.from_number))
        })
        .expect("a line with relays at both ends");
    let (a, b) = (line.from_number, line.to_number);
    let request = RmsRequest {
        t_end: 2.0,
        events: vec![RmsEvent::LineFault {
            time: 0.1,
            from: a,
            to: b,
            circuit: line.circuit.clone(),
            location: 0.5,
            r: 0.0,
            x: 1e-4,
            permanent: false,
        }],
        ..RmsRequest::default()
    };
    let (result, report) = sim.run_with_protection(&request, &set.scheme).unwrap();
    let trips: Vec<_> = report
        .operations
        .iter()
        .filter(|o| o.kind == OperationKind::Trip)
        .collect();
    println!(
        "{a}-{b}: {:?}",
        trips
            .iter()
            .map(|o| format!("{} {} {:.3}", o.relay, o.function, o.time))
            .collect::<Vec<_>>()
    );
    for end in [format!("L{a}-{b}"), format!("L{b}-{a}")] {
        let t = trips.iter().find(|o| o.relay == end).map(|o| o.time);
        assert!(
            t.is_some_and(|t| t > 0.1 && t < 0.2),
            "{end} trips within 100 ms of the fault: {t:?}"
        );
    }
    assert!(
        report
            .operations
            .iter()
            .any(|o| o.kind == OperationKind::Reclose),
        "transient fault: reclosed"
    );
    assert!(result
        .events
        .iter()
        .any(|e| e.contains("protection opened")));
}

/// The standard suite on the IEEE 14-bus case (case data), set on the
/// case's own ratings and on ratings estimated as the app does.
#[test]
fn ieee14_suite() {
    let Some(dir) = ieee14() else { return };
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    for (what, set) in [
        (
            "case ratings",
            auto_scheme(sim.topology(), &SettingOptions::default()),
        ),
        ("estimated ratings", set_by_rule(&sim)),
    ] {
        let report = run_suite(&sim, &set.scheme, &SuiteOptions::default());
        println!(
            "{what}: {} tests, {} passed, {} failed",
            report.tests.len(),
            report.passed,
            report.failed
        );
        print_failures(&report);
        assert_eq!(report.failed, 0, "{what}");
    }
}

/// Protection on the load flow layer: a case without dynamic data gets
/// relays and the operating point, and a static study clears line and bus
/// faults in the expected order (case data).
#[test]
fn static_study_without_dynamics() {
    use tellegen::protection::statics::{static_study, StaticFault};
    let Some(dir) = rms_example("ieee14_full") else {
        return;
    };
    let (net, _) = load(&dir);
    let none = powerio::parse_dyr("").unwrap();
    let sim = RmsSimulation::new(&net, &none, LoadModel::ConstantImpedance).unwrap();
    let set = set_by_rule(&sim);
    let op = sim.relays_at_operating_point(&set.scheme).unwrap();
    assert!(op.pickups.is_empty(), "{:?}", op.pickups);
    let study = static_study(
        sim.topology(),
        &set.scheme,
        &StaticFault::Line {
            from: 2,
            to: 4,
            circuit: "1".into(),
            location: 0.9,
            r: 0.0,
            x: 0.0,
        },
        3.0,
    )
    .unwrap();
    let first = |id: &str| study.trips.iter().find(|t| t.relay == id).map(|t| t.time);
    assert!(first("L4-2").is_some_and(|t| t < 0.03) && first("L2-4").is_some_and(|t| t < 0.05));
    assert!(study.cleared_at.is_some());
    let bus = static_study(
        sim.topology(),
        &set.scheme,
        &StaticFault::Bus {
            bus: 4,
            r: 0.0,
            x: 0.0,
        },
        3.0,
    )
    .unwrap();
    assert!(bus.trips.iter().any(|t| t.relay == "B4") && bus.cleared_at.is_some_and(|t| t < 0.1));
}

/// Faults on the intact network by superposition match the full solve
/// (WECC 240).
#[test]
fn fast_faults_match_full_solve() {
    use num_complex::Complex;
    use tellegen::protection::setting::{FaultCalc, FaultPoint};
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let topo = sim.topology();
    let calc = FaultCalc::new(topo);
    let e = topo
        .branches
        .iter()
        .position(|b| !b.transformer)
        .expect("a line");
    let bus = topo.branches[e].from;
    for (at, zf) in [
        (FaultPoint::Bus(bus), Complex::new(0.0, 0.0)),
        (FaultPoint::Branch(e, 0.3), Complex::new(0.01, 0.02)),
    ] {
        let fast = calc.solve_with(Some(at), zf, &[], &[]).unwrap();
        // A units-out entry that matches nothing forces the full solve.
        let full = calc.solve_with(Some(at), zf, &[], &[usize::MAX]).unwrap();
        let dv = fast
            .v
            .iter()
            .zip(&full.v)
            .map(|(a, b)| (a - b).norm())
            .fold(0.0, f64::max);
        let di = fast
            .branch
            .iter()
            .zip(&full.branch)
            .map(|(a, b)| (a.0 - b.0).norm().max((a.1 - b.1).norm()))
            .fold(0.0, f64::max);
        assert!(
            dv < 1e-7 && di < 1e-6 && (fast.fault_current - full.fault_current).norm() < 1e-6,
            "|ΔV| {dv:.1e}, |ΔI| {di:.1e}"
        );
    }
}

/// The standard suite on WECC 240 with the distance tests on every line.
#[test]
#[ignore = "about ten minutes in release; run with --include-ignored"]
fn wecc240_suite_every_line() {
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let set = set_by_rule(&sim);
    let report = run_suite(&sim, &set.scheme, &SuiteOptions::default());
    println!(
        "{} tests: {} passed, {} failed",
        report.tests.len(),
        report.passed,
        report.failed
    );
    print_failures(&report);
    assert_eq!(report.failed, 0);
}

/// Setting WECC 240 by rule, and what the relays cost an RMS run.
#[test]
#[ignore = "timing; run in release with --include-ignored"]
fn wecc240_setup_and_relay_cost() {
    use std::time::Instant;
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let t = Instant::now();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let t_build = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let set = set_by_rule(&sim);
    let t_set = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut rated = sim.topology().clone();
    estimate_ratings(&mut rated, &sim.branch_loading());
    let findings = audit(&rated, &set.scheme);
    let t_audit = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let op = sim.relays_at_operating_point(&set.scheme).unwrap();
    let t_op = t.elapsed().as_secs_f64();
    let request = RmsRequest {
        t_end: 1.0,
        ..RmsRequest::default()
    };
    let t = Instant::now();
    sim.run(&request).unwrap();
    let t_plain = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let (_, report) = sim.run_with_protection(&request, &set.scheme).unwrap();
    let t_relays = t.elapsed().as_secs_f64();
    let acted = report
        .operations
        .iter()
        .filter(|o| !matches!(o.kind, OperationKind::Pickup | OperationKind::Reset))
        .count();
    println!(
        "WECC 240: {} buses, {} branches, {} relays, {} banks",
        sim.topology().bus_numbers.len(),
        sim.topology().branches.len(),
        set.scheme.relays.len(),
        set.scheme.series_capacitors.len()
    );
    println!(
        "build {t_build:.2} s, setting {t_set:.2} s, audit {t_audit:.2} s ({} findings), operating point {t_op:.2} s ({} pickups)",
        findings.len(),
        op.pickups.len()
    );
    println!(
        "1 s at rest: {t_plain:.2} s without relays, {t_relays:.2} s with them ({:.1}×); {acted} operations",
        t_relays / t_plain
    );
}

/// A live session carries relays: a transient line fault applied while it
/// runs trips both ends and recloses (WECC 240).
#[cfg(feature = "rms-experimental")]
#[test]
fn live_session_carries_relays() {
    let dir = rms_example("wecc240").expect("WECC 240 ships with the code");
    let (net, dyr) = load(&dir);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let set = set_by_rule(&sim);
    let mut live = sim.live(0.0025, 0.02);
    live.set_protection(Some(&set.scheme)).unwrap();
    let rest = live.advance(0.2).unwrap();
    assert!(
        rest.relays
            .iter()
            .all(|o| matches!(o.kind, OperationKind::Pickup | OperationKind::Reset)),
        "nothing acts at rest: {:?}",
        rest.relays
    );
    live.apply(RmsEvent::LineFault {
        time: 0.0,
        from: 1001,
        to: 1201,
        circuit: "1".into(),
        location: 0.5,
        r: 0.0,
        x: 1e-5,
        permanent: false,
    })
    .unwrap();
    let mut ops = Vec::new();
    for _ in 0..50 {
        ops.extend(live.advance(0.02).unwrap().relays);
    }
    let trips: Vec<_> = ops
        .iter()
        .filter(|o| o.kind == OperationKind::Trip)
        .collect();
    assert!(
        trips.iter().any(|o| o.relay == "L1001-1201")
            && trips.iter().any(|o| o.relay == "L1201-1001"),
        "both ends trip: {:?}",
        trips
            .iter()
            .map(|o| format!("{} {} {:.3}", o.relay, o.function, o.time))
            .collect::<Vec<_>>()
    );
    assert!(
        ops.iter().any(|o| o.kind == OperationKind::Reclose),
        "transient fault: reclosed"
    );
}
