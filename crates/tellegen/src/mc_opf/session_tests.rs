use super::*;
use crate::mc_opf::{
    initial_tests::case,
    tests::{network, rich_network},
};

fn session(net: powerio::MulticonductorNetwork) -> McOpfSession {
    McOpfSession::new(
        McAcOpfInstance::from_network(net).unwrap(),
        McOpfOptions::default(),
    )
    .unwrap()
}
fn load(p: f64) -> McOpfEdit {
    McOpfEdit::LoadPower {
        load: "d".into(),
        p_w: vec![p],
        q_var: vec![0.],
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5 * (1. + b.abs()), "{a} != {b}");
}
fn compare_fresh(s: &McOpfSession) {
    let retained = &s.current_result().unwrap().result;
    let fresh = solve_mc_ac_opf_instance(Arc::new(s.instance().clone()), s.options()).unwrap();
    near(retained.solution.objective(), fresh.solution.objective());
    assert_eq!(retained.fingerprint, fresh.fingerprint);
    assert!(retained.residuals.maximum() <= s.options().acceptance_tolerance);
    assert!(fresh.residuals.maximum() <= s.options().acceptance_tolerance);
}

#[test]
fn repeated_solves_reuse_the_compilation_and_carry_an_accepted_primal() {
    let mut s = session(rich_network());
    assert_eq!(s.revision(), 0);
    assert!(s.current_result().is_none());
    assert_eq!(s.solve().unwrap().start, McOpfStartUsed::Declared);
    let first = s.current_result().unwrap().result.solution.objective();
    assert_eq!(s.solve().unwrap().start, McOpfStartUsed::PreviousPrimal);
    assert_eq!(s.compilation_count(), 1);
    assert_eq!(s.solve_count(), 2);
    near(
        s.current_result().unwrap().result.solution.objective(),
        first,
    );
    compare_fresh(&s);
}

#[test]
fn physical_edits_match_fresh_solves_and_preserve_the_original_input() {
    let net = rich_network();
    let mut s = session(net.clone());
    s.solve().unwrap();
    let old = s.last_accepted().unwrap().result.clone();
    for p in [160., 80., 120.] {
        let before = s.compilation_count();
        s.apply_edits(&[load(p)]).unwrap();
        assert!(s.current_result().is_none());
        assert_eq!(s.compilation_count(), before + 1);
        near(s.instance().network().loads()[0].p_nom[0], p);
        assert_eq!(s.solve().unwrap().start, McOpfStartUsed::PreviousPrimal);
        let load = s
            .current_result()
            .unwrap()
            .result
            .devices
            .iter()
            .find(|d| d.identity == "d")
            .unwrap();
        near(load.coil_power_va[0][0], p);
        compare_fresh(&s);
    }
    near(net.loads()[0].p_nom[0], 100.);
    near(old.solution.instance().network().loads()[0].p_nom[0], 100.);
    let read = McOpfResult::from_module_json(
        &s.current_result().unwrap().result.to_module_json().unwrap(),
    )
    .unwrap();
    near(read.solution.instance().network().loads()[0].p_nom[0], 120.);
    assert_eq!(
        read.fingerprint,
        s.current_result().unwrap().result.fingerprint
    );
}

#[test]
fn costs_dispatch_limits_and_constraint_selection_have_physical_effects() {
    let mut s = session(rich_network());
    s.solve().unwrap();
    let generation = |s: &McOpfSession| {
        s.current_result()
            .unwrap()
            .result
            .devices
            .iter()
            .find(|d| d.identity == "g")
            .unwrap()
            .coil_power_va[0][0]
    };
    assert!(generation(&s) > 49.);
    s.apply_edits(&[McOpfEdit::GeneratorCost {
        generator: "g".into(),
        cost_per_kwh: Some(vec![1.]),
    }])
    .unwrap();
    s.solve().unwrap();
    assert!(generation(&s) < 1e-3);
    compare_fresh(&s);
    s.apply_edits(&[
        McOpfEdit::SourceCost {
            source: "grid".into(),
            cost_per_kwh: Some(vec![2.]),
        },
        McOpfEdit::GeneratorPowerBounds {
            generator: "g".into(),
            p_min_w: Some(vec![0.]),
            p_max_w: Some(vec![25.]),
            q_min_var: Some(vec![-25.]),
            q_max_var: Some(vec![25.]),
        },
    ])
    .unwrap();
    s.solve().unwrap();
    near(generation(&s), 25.);
    compare_fresh(&s);
    let mut constraints = s.instance().constraints().clone();
    constraints.generator_capability = powerio_prob::ConstraintSelection::None;
    // Equal prices keep the relaxed dispatch problem bounded; check selection
    // through its fresh compiled limits rather than a nonunique optimum.
    s.apply_edits(&[
        McOpfEdit::ConstraintSelection { constraints },
        McOpfEdit::GeneratorCost {
            generator: "g".into(),
            cost_per_kwh: Some(vec![2.]),
        },
    ])
    .unwrap();
    let prep = initial::prepare(s.instance(), &Default::default()).unwrap();
    let g = prep.devices.iter().find(|d| d.identity == "g").unwrap();
    assert!(g
        .coils
        .iter()
        .all(|c| c.p_min.is_none() && c.p_max.is_none()));
    s.solve().unwrap();
    compare_fresh(&s);
}

#[test]
fn availability_edits_include_zero_and_do_not_become_seed_constraints() {
    let net = case("components", "ibr_single_phase");
    let id = net.ibrs()[0].name.clone();
    let mut s = session(net);
    s.solve().unwrap();
    for available in [Some(0.), Some(30.), Some(10.), None] {
        s.apply_edits(&[McOpfEdit::InverterAvailability {
            inverter: id.clone(),
            p_max_w: available.map(|v| vec![v]),
        }])
        .unwrap();
        s.solve().unwrap();
        assert_eq!(s.instance().network().ibrs()[0].p_avail, available);
        if let Some(available) = available {
            let d = s
                .current_result()
                .unwrap()
                .result
                .devices
                .iter()
                .find(|d| d.identity == id)
                .unwrap();
            assert!(d.coil_power_va.iter().map(|p| p[0]).sum::<f64>() <= available + 1e-3);
        }
        compare_fresh(&s);
    }
}

#[test]
fn crossing_zero_ratings_recompiles_and_resets_proven_zero_coordinates() {
    for apparent in [false, true] {
        let mut s = session(network());
        s.solve().unwrap();
        let old = s.current_result().unwrap().result.fingerprint.clone();
        let limits = |limit| McOpfEdit::LineLimits {
            line: "l".into(),
            current_a: (!apparent).then(|| vec![limit; 2]),
            apparent_power_va: apparent.then(|| vec![limit; 2]),
        };
        s.apply_edits(&[load(0.), limits(0.)]).unwrap();
        assert_eq!(s.compilation_count(), 2);
        assert!(!s.model.zero_columns.is_empty());
        let mut warm = s.model.clone();
        s.initialize(&mut warm, McOpfStart::LastAccepted).unwrap();
        for &i in &warm.zero_columns {
            assert_eq!(warm.problem.x0[i], 0.);
        }
        s.solve().unwrap();
        assert_ne!(old, s.current_result().unwrap().result.fingerprint);
        compare_fresh(&s);
        s.apply_edits(&[load(100.), limits(if apparent { 1000. } else { 5. })])
            .unwrap();
        assert_eq!(s.compilation_count(), 3);
        s.solve().unwrap();
        compare_fresh(&s);
    }
}

#[test]
fn edit_batches_and_option_updates_are_transactional_on_invalid_input() {
    let mut s = session(rich_network());
    s.solve().unwrap();
    let before = s.current_result().unwrap().result.fingerprint.clone();
    let bad = [
        McOpfEdit::LoadPower {
            load: "missing".into(),
            p_w: vec![1.],
            q_var: vec![0.],
        },
        McOpfEdit::LoadPower {
            load: "d".into(),
            p_w: vec![f64::NAN],
            q_var: vec![0.],
        },
        McOpfEdit::LoadPower {
            load: "d".into(),
            p_w: vec![10., 20.],
            q_var: vec![0.],
        },
        McOpfEdit::LineLimits {
            line: "l".into(),
            current_a: Some(vec![-1.; 2]),
            apparent_power_va: None,
        },
        McOpfEdit::BusVoltageBounds {
            bus: "b".into(),
            minimum_v: Some(250.),
            maximum_v: Some(200.),
        },
        // This survives local shape checks, then fails PowerIO preparation.
        McOpfEdit::SourceCost {
            source: "grid".into(),
            cost_per_kwh: Some(vec![0.2; 7]),
        },
    ];
    for edit in bad {
        assert_eq!(
            s.apply_edits(&[load(999.), edit]).unwrap_err().kind,
            McOpfErrorKind::InvalidInput
        );
        assert_eq!(s.revision(), 0);
        assert_eq!(s.compilation_count(), 1);
        assert_eq!(s.current_result().unwrap().result.fingerprint, before);
        near(s.instance().network().loads()[0].p_nom[0], 100.);
    }
    let mut options = s.options().clone();
    options.voltage_base_v = f64::INFINITY;
    assert!(s.set_options(options).is_err());
    assert!(s.current_result().is_some());
    assert_eq!(s.apply_edits(&[]).unwrap(), 0);
    assert_eq!(s.set_options(s.options().clone()).unwrap(), 0);
}

#[test]
fn cancellation_and_numerical_failure_preserve_history_but_never_current_detail() {
    let mut s = session(network());
    s.solve().unwrap();
    let accepted = s.last_accepted().unwrap().attempt;
    let primal = s.primal.clone();
    let cancel = Arc::new(AtomicBool::new(true));
    let error = s
        .solve_cancellable(McOpfStart::LastAccepted, Some(cancel))
        .unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::Cancelled);
    assert!(s.current_result().is_none());
    assert_eq!(s.last_accepted().unwrap().attempt, accepted);
    assert_eq!(s.primal, primal);
    s.apply_edits(&[load(200.)]).unwrap();
    let mut options = s.options().clone();
    options.max_iterations = 1;
    s.set_options(options).unwrap();
    let error = s.solve_cancellable(McOpfStart::Declared, None).unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::NumericalFailure);
    assert!(s.current_result().is_none());
    assert_eq!(s.last_accepted().unwrap().attempt, accepted);
    assert_eq!(s.primal, primal);
    s.set_options(McOpfOptions::default()).unwrap();
    assert_eq!(s.solve().unwrap().start, McOpfStartUsed::PreviousPrimal);
    compare_fresh(&s);
}

#[test]
fn infeasible_edited_revision_cannot_publish_the_old_solution() {
    let mut s = session(network());
    s.solve().unwrap();
    s.apply_edits(&[McOpfEdit::LineLimits {
        line: "l".into(),
        current_a: Some(vec![0.; 2]),
        apparent_power_va: None,
    }])
    .unwrap();
    assert!(s.solve().is_err());
    assert!(s.current_result().is_none());
    assert_eq!(s.last_accepted().unwrap().revision, 0);
    assert_eq!(s.revision(), 1);
    s.apply_edits(&[McOpfEdit::LineLimits {
        line: "l".into(),
        current_a: None,
        apparent_power_va: None,
    }])
    .unwrap();
    s.solve().unwrap();
    assert_eq!(s.current_result().unwrap().revision, 2);
    compare_fresh(&s);
}

#[test]
fn failed_independent_acceptance_never_replaces_the_retained_primal() {
    let mut s = session(rich_network());
    s.solve().unwrap();
    let accepted = s.last_accepted().unwrap().attempt;
    let primal = s.primal.clone();
    let mut options = s.options().clone();
    // The optimizer can converge while failing a stricter independent gate.
    options.acceptance_tolerance = 1e-30;
    s.set_options(options).unwrap();
    let error = s.solve_cancellable(McOpfStart::Declared, None).unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::PhysicalValidationFailure);
    assert!(s.current_result().is_none());
    assert_eq!(s.last_accepted().unwrap().attempt, accepted);
    assert_eq!(s.primal, primal);
    s.set_options(McOpfOptions::default()).unwrap();
    s.solve().unwrap();
    compare_fresh(&s);
}

#[test]
fn base_changes_project_si_voltages_and_taps_and_solver_options_reuse_the_model() {
    let mut s = session(case("controls", "tap_regulator_b"));
    s.solve().unwrap();
    let old = s.last_accepted().unwrap().result.clone();
    let mut options = s.options().clone();
    options.collect_profile = true;
    options.objective_scale = 2.;
    s.set_options(options.clone()).unwrap();
    assert_eq!(s.compilation_count(), 1);
    assert!(s.current_result().is_none());
    s.solve().unwrap();
    assert_eq!(
        s.current_result().unwrap().start,
        McOpfStartUsed::PreviousPrimal
    );
    options.voltage_base_v = 100.;
    options.power_base_va = 3000.;
    s.set_options(options).unwrap();
    assert_eq!(s.compilation_count(), 2);
    let mut warm = s.model.clone();
    assert_eq!(
        s.initialize(&mut warm, McOpfStart::LastAccepted).unwrap().0,
        McOpfStartUsed::PreviousOperatingPoint
    );
    for (id, pair) in warm.prep.terminals.iter().zip(&warm.voltage) {
        let expected = old
            .terminals
            .iter()
            .find(|t| t.terminal.bus == id.bus && t.terminal.terminal == id.terminal)
            .unwrap();
        near(
            pair[0].value(&warm.problem.x0) * 100.,
            expected.voltage_v[0],
        );
        near(
            pair[1].value(&warm.problem.x0) * 100.,
            expected.voltage_v[1],
        );
    }
    let t = &warm.prep.transformers[0];
    near(
        warm.problem.x0[warm.transformer_tap[0].unwrap()],
        t.tap_nominal / old.transformers[0].tap.unwrap(),
    );
    s.solve().unwrap();
    compare_fresh(&s);
    assert!(s
        .current_result()
        .unwrap()
        .result
        .profile
        .as_ref()
        .unwrap()
        .stages_s
        .contains_key("session_model_clone"));
}

#[test]
fn declared_start_is_explicit_and_warm_initialization_leaves_the_cached_model_untouched() {
    let net = network();
    let point = MulticonductorOperatingPointBuilder::for_point(net.clone())
        .terminal_voltage_magnitudes(vec![230., 0., 150., 0.])
        .build_point()
        .unwrap();
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(net)
            .unwrap()
            .with_initial_point(point),
        Default::default(),
    )
    .unwrap();
    let cold = s.model.problem.x0.clone();
    s.solve().unwrap();
    let mut warm = s.model.clone();
    s.initialize(&mut warm, McOpfStart::LastAccepted).unwrap();
    assert_ne!(warm.problem.x0, cold);
    for (name, value) in warm.problem.var_names.iter().zip(&warm.problem.x0) {
        if name.contains("voltage") {
            near(*value, s.primal[name]);
        }
    }
    assert_eq!(s.model.problem.x0, cold);
    assert_eq!(
        s.solve_cancellable(McOpfStart::Declared, None)
            .unwrap()
            .start,
        McOpfStartUsed::Declared
    );
    near(
        s.current_result()
            .unwrap()
            .result
            .solution
            .instance()
            .initial_point()
            .unwrap()
            .terminal_voltage_magnitude("b", "a")
            .unwrap(),
        150.,
    );
}

#[test]
fn zip_and_delta_load_edits_keep_coil_semantics_and_fresh_auxiliary_values() {
    for case_name in ["load_zip", "capacitor_delta"] {
        let mut net = case("components", case_name);
        net.buses_mut().reverse();
        net.loads_mut().reverse();
        for bus in net.buses_mut() {
            bus.terminals.reverse();
        }
        let mut s = session(net);
        s.solve().unwrap();
        let load = s
            .instance()
            .network()
            .loads()
            .iter()
            .find(|d| {
                if case_name == "capacitor_delta" {
                    d.configuration == powerio_dist::Configuration::Delta
                } else {
                    matches!(
                        d.voltage_model,
                        powerio_dist::DistLoadVoltageModel::Zip { .. }
                    )
                }
            })
            .unwrap();
        let id = load.name.clone();
        let p: Vec<_> = load.p_nom.iter().map(|p| p * 0.8).collect();
        let q: Vec<_> = load.q_nom.iter().map(|q| q * 0.7).collect();
        let law = load.voltage_model.clone();
        s.apply_edits(&[McOpfEdit::LoadPower {
            load: id.clone(),
            p_w: p.clone(),
            q_var: q.clone(),
        }])
        .unwrap();
        let load = s
            .instance()
            .network()
            .loads()
            .iter()
            .find(|d| d.name == id)
            .unwrap();
        assert_eq!(load.p_nom, p);
        assert_eq!(load.q_nom, q);
        assert_eq!(load.voltage_model, law);
        s.solve().unwrap();
        compare_fresh(&s);
    }
}

#[test]
fn availability_allocation_is_explicit_per_coil_and_survives_export() {
    let mut s = session(case("components", "ibr_four_leg"));
    s.solve().unwrap();
    let id = s.instance().network().ibrs()[0].name.clone();
    let caps = vec![0., 20., 40.];
    s.apply_edits(&[McOpfEdit::InverterAvailability {
        inverter: id.clone(),
        p_max_w: Some(caps.clone()),
    }])
    .unwrap();
    s.solve().unwrap();
    let r = &s.current_result().unwrap().result;
    let inv = r.devices.iter().find(|d| d.identity == id).unwrap();
    for (p, cap) in inv.coil_power_va.iter().zip(&caps) {
        assert!(p[0] <= cap + 1e-3);
    }
    let read = McOpfResult::from_module_json(&r.to_module_json().unwrap()).unwrap();
    assert_eq!(
        read.solution.instance().network().ibrs()[0]
            .p_max
            .as_ref()
            .unwrap(),
        &caps
    );
    assert_eq!(
        read.solution.instance().network().ibrs()[0].p_avail,
        Some(60.)
    );
    compare_fresh(&s);
    let revision = s.revision();
    for caps in [vec![1.], vec![-1.; 3], vec![f64::MAX; 3]] {
        assert!(s
            .apply_edits(&[McOpfEdit::InverterAvailability {
                inverter: id.clone(),
                p_max_w: Some(caps)
            }])
            .is_err());
        assert_eq!(s.revision(), revision);
        assert!(s.current_result().is_some());
    }
}

#[test]
fn voltage_limit_edits_and_removing_line_overrides_match_canonical_fallbacks() {
    let mut s = session(rich_network());
    s.solve().unwrap();
    s.apply_edits(&[
        McOpfEdit::BusVoltageBounds {
            bus: "b".into(),
            minimum_v: Some(220.),
            maximum_v: Some(240.),
        },
        McOpfEdit::LineLimits {
            line: "l".into(),
            current_a: Some(vec![5.; 2]),
            apparent_power_va: None,
        },
    ])
    .unwrap();
    s.solve().unwrap();
    compare_fresh(&s);
    near(s.instance().network().buses()[1].v_min.unwrap(), 220.);
    s.apply_edits(&[
        McOpfEdit::BusVoltageBounds {
            bus: "b".into(),
            minimum_v: None,
            maximum_v: None,
        },
        McOpfEdit::LineLimits {
            line: "l".into(),
            current_a: None,
            apparent_power_va: None,
        },
    ])
    .unwrap();
    assert_eq!(
        s.model.prep.branches[0].current_max[0],
        Some(20. / (1000. / 230.))
    );
    s.solve().unwrap();
    compare_fresh(&s);
}

#[test]
fn unsupported_initial_fields_are_rejected_before_session_normalization() {
    let net = network();
    let point = MulticonductorOperatingPointBuilder::for_point(net.clone())
        .switch_closed(vec![])
        .build_point()
        .unwrap();
    let error = McOpfSession::new(
        McAcOpfInstance::from_network(net)
            .unwrap()
            .with_initial_point(point),
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, McOpfErrorKind::UnsupportedInput);
}
