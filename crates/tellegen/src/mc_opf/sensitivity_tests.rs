use super::*;
use powerio::MulticonductorNetwork;
use powerio_matrix::McOpfDeviceKind as DeviceKind;
use McOpfFlowQuantity as Flow;
use McOpfParameterKind as Kind;
use McOpfParameterOwner as Owner;
use McOpfSensitivityOutput as Output;

pub(super) fn solve(net: MulticonductorNetwork) -> McOpfResult {
    solve_options(
        net,
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
}
pub(super) fn solve_options(net: MulticonductorNetwork, options: McOpfOptions) -> McOpfResult {
    solve_mc_ac_opf_instance(
        Arc::new(McAcOpfInstance::from_network(net).unwrap()),
        &options,
    )
    .unwrap()
}
fn parameter(owner: Owner, id: &str, kind: Kind) -> McOpfParameter {
    McOpfParameter::new(owner, id, Some(0), kind)
}
fn load(reactive: bool) -> McOpfParameter {
    parameter(
        Owner::Load,
        "d",
        if reactive {
            Kind::LoadReactivePower
        } else {
            Kind::LoadActivePower
        },
    )
}
fn voltage(bus: &str, terminal: &str, part: McOpfVoltagePart) -> Output {
    Output::Voltage {
        terminal: McOpfTerminalId {
            bus: bus.into(),
            terminal: terminal.into(),
        },
        part,
    }
}
fn device(kind: DeviceKind, id: &str, axis: McOpfDeviceAxis, quantity: Flow) -> Output {
    Output::Device {
        kind,
        identity: id.into(),
        axis,
        quantity,
    }
}
fn branch(end: McOpfBranchEnd, conductor: usize, quantity: Flow) -> Output {
    Output::Branch {
        identity: "line:l".into(),
        end,
        conductor,
        quantity,
    }
}
pub(super) fn outputs() -> Vec<Output> {
    let mut v = vec![Output::Objective];
    for t in ["a", "n"] {
        for part in [
            McOpfVoltagePart::Real,
            McOpfVoltagePart::Imaginary,
            McOpfVoltagePart::Magnitude,
            McOpfVoltagePart::Angle,
        ] {
            v.push(voltage("b", t, part));
        }
        for quantity in [
            Flow::ActivePower,
            Flow::ReactivePower,
            Flow::CurrentReal,
            Flow::CurrentImaginary,
        ] {
            v.push(device(
                DeviceKind::Load,
                "d",
                McOpfDeviceAxis::Terminal { terminal: t.into() },
                quantity,
            ));
        }
    }
    for end in [McOpfBranchEnd::From, McOpfBranchEnd::To] {
        for c in 0..2 {
            for q in [
                Flow::ActivePower,
                Flow::ReactivePower,
                Flow::CurrentReal,
                Flow::CurrentImaginary,
            ] {
                v.push(branch(end, c, q));
            }
        }
    }
    v.push(device(
        DeviceKind::Source,
        "grid",
        McOpfDeviceAxis::Coil { index: 0 },
        Flow::ActivePower,
    ));
    v
}
pub(super) fn physical(r: &McOpfResult, output: &Output) -> f64 {
    let complex = |current: [f64; 2], power: [f64; 2], q: &Flow| match q {
        Flow::CurrentReal => current[0],
        Flow::CurrentImaginary => current[1],
        Flow::ActivePower => power[0],
        Flow::ReactivePower => power[1],
    };
    match output {
        Output::Objective => r.solution.objective(),
        Output::Voltage { terminal, part } => {
            let v = r
                .terminals
                .iter()
                .find(|t| t.terminal == *terminal)
                .unwrap()
                .voltage_v;
            match part {
                McOpfVoltagePart::Real => v[0],
                McOpfVoltagePart::Imaginary => v[1],
                McOpfVoltagePart::Magnitude => v[0].hypot(v[1]),
                McOpfVoltagePart::Angle => v[1].atan2(v[0]),
            }
        }
        Output::Device {
            kind,
            identity,
            axis,
            quantity,
        } => {
            let d = r
                .devices
                .iter()
                .find(|d| d.kind == *kind && d.identity == *identity)
                .unwrap();
            let (i, p) = match axis {
                McOpfDeviceAxis::Coil { index } => {
                    (d.coil_current_a[*index], d.coil_power_va[*index])
                }
                McOpfDeviceAxis::Terminal { terminal } => {
                    let k = d
                        .terminals
                        .iter()
                        .position(|t| t.terminal == *terminal)
                        .unwrap();
                    (d.terminal_current_a[k], d.terminal_power_va[k])
                }
            };
            complex(i, p, quantity)
        }
        Output::Branch {
            identity,
            end,
            conductor,
            quantity,
        } => {
            let b = r.branches.iter().find(|b| b.identity == *identity).unwrap();
            let k = *conductor;
            let (i, p) = match end {
                McOpfBranchEnd::From => (b.current_from_a[k], b.power_from_va[k]),
                McOpfBranchEnd::To => (b.current_to_a[k], b.power_to_va[k]),
            };
            complex(i, p, quantity)
        }
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8 + 3e-4 * b.abs(), "{a} != {b}");
}
fn one_bus() -> MulticonductorNetwork {
    let mut n = tests::network();
    n.buses_mut().truncate(1);
    n.lines_mut().clear();
    n.line_codes_mut().clear();
    n.loads_mut()[0].bus = "s".into();
    n
}
pub(super) fn check_fd(
    net: MulticonductorNetwork,
    parameter: McOpfParameter,
    outputs: &[Output],
    step: f64,
    edit: impl Fn(&mut MulticonductorNetwork, f64),
) {
    let r = solve(net.clone());
    let s = r.sensitivities().unwrap();
    let f = s
        .forward(
            &[McOpfParameterDirection {
                parameter: parameter.clone(),
                delta: 1.,
            }],
            outputs,
        )
        .unwrap();
    let mut plus = net.clone();
    let mut minus = net;
    edit(&mut plus, step);
    edit(&mut minus, -step);
    let (plus, minus) = (solve(plus), solve(minus));
    for (o, v) in outputs.iter().zip(f) {
        let fd = (physical(&plus, o) - physical(&minus, o)) / (2. * step);
        assert!(
            (v.derivative - fd).abs() < 2e-8 + 3e-4 * fd.abs(),
            "{o:?}: {} != {fd}",
            v.derivative
        );
        let a = s
            .adjoint(
                &[McOpfOutputWeight {
                    output: o.clone(),
                    weight: 1.,
                }],
                std::slice::from_ref(&parameter),
            )
            .unwrap();
        close(v.derivative, a[0].derivative);
        if *o == Output::Objective {
            let marginal = r
                .duals
                .as_ref()
                .unwrap()
                .marginals
                .iter()
                .find(|m| m.parameter == parameter)
                .unwrap();
            close(v.derivative, marginal.derivative.unwrap());
        }
    }
}

#[test]
fn analytic_lossless_forward_adjoint_and_objective_marginals_agree() {
    let r = solve(one_bus());
    let s = r.sensitivities().unwrap();
    let out = vec![
        Output::Objective,
        voltage("s", "a", McOpfVoltagePart::Real),
        device(
            DeviceKind::Source,
            "grid",
            McOpfDeviceAxis::Coil { index: 0 },
            Flow::ActivePower,
        ),
    ];
    let f = s
        .forward(
            &[McOpfParameterDirection {
                parameter: load(false),
                delta: 1.,
            }],
            &out,
        )
        .unwrap();
    close(f[0].derivative, 0.0002);
    close(f[1].derivative, 0.);
    close(f[2].derivative, 1.);
    assert_eq!(f[0].unit, McOpfOutputUnit::CurrencyPerHour);
    let a = s
        .adjoint(
            &[McOpfOutputWeight {
                output: Output::Objective,
                weight: 1.,
            }],
            &[load(false), load(true)],
        )
        .unwrap();
    close(a[0].derivative, 0.0002);
    close(a[1].derivative, 0.);
    for v in a {
        let m = r
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == v.parameter)
            .unwrap();
        close(v.derivative, m.derivative.unwrap());
    }
}

#[test]
fn lossy_p_q_sensitivities_match_perturbed_physical_ledgers() {
    let mut n = tests::network();
    n.loads_mut()[0].q_nom[0] = 30.;
    n.line_codes_mut()[0].x_series = vec![vec![0.05, 0.01], vec![0.01, 0.05]];
    for reactive in [false, true] {
        check_fd(n.clone(), load(reactive), &outputs(), 0.1, |n, d| {
            if reactive {
                n.loads_mut()[0].q_nom[0] += d;
            } else {
                n.loads_mut()[0].p_nom[0] += d;
            }
        });
    }
}

fn constrained() -> MulticonductorNetwork {
    let mut n = tests::rich_network();
    n.generators_mut()[0].cost = Some(vec![1.]);
    n.generators_mut()[0].p_max = Some(vec![150.]);
    n
}
#[test]
fn binding_current_and_apparent_ratings_include_both_ends_and_mixed_derivatives() {
    for apparent in [false, true] {
        let mut n = constrained();
        if apparent {
            n.lines_mut()[0].s_max = Some(vec![46., 1000.]);
        } else {
            n.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
        }
        let kind = if apparent {
            Kind::ApparentPowerRating
        } else {
            Kind::CurrentRating
        };
        let mut out = outputs();
        out.push(device(
            DeviceKind::Generator,
            "g",
            McOpfDeviceAxis::Coil { index: 0 },
            Flow::ActivePower,
        ));
        check_fd(
            n,
            parameter(Owner::Branch, "line:l", kind),
            &out,
            if apparent { 0.01 } else { 0.0001 },
            |n, d| {
                if apparent {
                    n.lines_mut()[0].s_max.as_mut().unwrap()[0] += d;
                } else {
                    n.lines_mut()[0].i_max.as_mut().unwrap()[0] += d;
                }
            },
        );
    }
}
#[test]
fn binding_voltage_floor_and_generator_limits_match_perturbations() {
    let mut n = constrained();
    n.buses_mut()[1].vpn_min = Some(vec![229.96]);
    let r = solve(n.clone());
    let p = r
        .duals
        .as_ref()
        .unwrap()
        .marginals
        .iter()
        .find(|m| m.parameter.kind == Kind::VoltageLower && m.derivative.is_some_and(|v| v > 1e-5))
        .unwrap()
        .parameter
        .clone();
    check_fd(n, p, &outputs(), 0.0001, |n, d| {
        n.buses_mut()[1].vpn_min.as_mut().unwrap()[0] += d
    });
    let n = tests::rich_network();
    let out = [
        Output::Objective,
        device(
            DeviceKind::Generator,
            "g",
            McOpfDeviceAxis::Coil { index: 0 },
            Flow::ActivePower,
        ),
    ];
    check_fd(
        n,
        parameter(Owner::Generator, "g", Kind::ActivePowerUpper),
        &out,
        0.1,
        |n, d| n.generators_mut()[0].p_max.as_mut().unwrap()[0] += d,
    );
    let mut n = tests::rich_network();
    n.generators_mut()[0].cost = Some(vec![1.]);
    n.generators_mut()[0].p_min = Some(vec![20.]);
    check_fd(
        n,
        parameter(Owner::Generator, "g", Kind::ActivePowerLower),
        &out,
        0.1,
        |n, d| n.generators_mut()[0].p_min.as_mut().unwrap()[0] += d,
    );
    let mut n = tests::rich_network();
    n.loads_mut()[0].q_nom[0] = 30.;
    n.generators_mut()[0].q_max = Some(vec![10.]);
    // At the original low loss level the optimizer has not resolved this
    // weak economic bound to the active-side tolerance. Do not differentiate
    // a truncated active set that fails stationarity.
    assert_eq!(
        solve(n.clone()).sensitivities().unwrap_err().dual_status,
        Some(McOpfMarginalStatus::FailedKktCheck)
    );
    n.line_codes_mut()[0].r_series = vec![vec![10., 0.02], vec![0.02, 10.]];
    let reactive_out = [
        Output::Objective,
        device(
            DeviceKind::Generator,
            "g",
            McOpfDeviceAxis::Coil { index: 0 },
            Flow::ReactivePower,
        ),
    ];
    check_fd(
        n,
        parameter(Owner::Generator, "g", Kind::ReactivePowerUpper),
        &reactive_out,
        0.1,
        |n, d| n.generators_mut()[0].q_max.as_mut().unwrap()[0] += d,
    );
}

#[test]
fn combined_directions_weighted_adjoints_and_inactive_parameters_are_consistent() {
    let r = solve(tests::rich_network());
    let s = r.sensitivities().unwrap();
    let params = [
        load(false),
        load(true),
        parameter(Owner::Branch, "line:l", Kind::CurrentRating),
    ];
    let out = [
        Output::Objective,
        voltage("b", "a", McOpfVoltagePart::Real),
        branch(McOpfBranchEnd::To, 1, Flow::CurrentReal),
    ];
    let direction: Vec<_> = params
        .iter()
        .zip([2., -3., 4.])
        .map(|(p, delta)| McOpfParameterDirection {
            parameter: p.clone(),
            delta,
        })
        .collect();
    let weights: Vec<_> = out
        .iter()
        .zip([1., 2., 3.])
        .map(|(o, weight)| McOpfOutputWeight {
            output: o.clone(),
            weight,
        })
        .collect();
    let f = s.forward(&direction, &out).unwrap();
    let a = s.adjoint(&weights, &params).unwrap();
    close(
        f.iter()
            .zip(&weights)
            .map(|(f, w)| f.derivative * w.weight)
            .sum(),
        a.iter()
            .zip(&direction)
            .map(|(a, d)| a.derivative * d.delta)
            .sum(),
    );
    close(a[2].derivative, 0.);
    let zero = s.forward(&[], &out).unwrap();
    assert!(zero.iter().all(|v| v.derivative == 0.));
}

#[test]
fn sensitivity_views_require_current_verified_regular_duals() {
    let cold = solve_options(tests::network(), Default::default());
    assert_eq!(
        cold.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::MissingDuals
    );
    let mut bad = solve(tests::network());
    bad.duals.as_mut().unwrap().constraints[0].multiplier += 1.;
    assert_eq!(
        bad.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::InvalidResult
    );
    let mut zero = tests::network();
    zero.loads_mut()[0].p_nom[0] = 0.;
    zero.lines_mut()[0].i_max = Some(vec![0.; 2]);
    let r = solve(zero);
    let e = r.sensitivities().unwrap_err();
    assert_eq!(e.kind, McOpfSensitivityErrorKind::UnreliableDuals);
    assert_eq!(e.dual_status, Some(McOpfMarginalStatus::ZeroLimitReduction));
    let mut n = constrained();
    n.line_codes_mut()[0].g_from = vec![vec![0.; 2]; 2];
    n.line_codes_mut()[0].b_to = vec![vec![0.; 2]; 2];
    n.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
    assert_eq!(
        solve(n).sensitivities().unwrap_err().dual_status,
        Some(McOpfMarginalStatus::RankDeficient)
    );
}

#[test]
fn session_edits_invalidate_current_sensitivities_but_keep_explicit_snapshots() {
    let mut session = McOpfSession::new(
        McAcOpfInstance::from_network(tests::network()).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        session.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
    session.solve().unwrap();
    let old = session.current_result().unwrap().result.clone();
    let f = |s: McOpfSensitivities<'_>| {
        s.forward(
            &[McOpfParameterDirection {
                parameter: load(false),
                delta: 1.,
            }],
            &[Output::Objective],
        )
        .unwrap()[0]
            .derivative
    };
    close(
        f(session.sensitivities().unwrap()),
        f(old.sensitivities().unwrap()),
    );
    session
        .apply_edits(&[McOpfEdit::LoadPower {
            load: "d".into(),
            p_w: vec![200.],
            q_var: vec![0.],
        }])
        .unwrap();
    assert_eq!(
        session.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
    session.solve().unwrap();
    let fresh = solve(session.instance().network().clone());
    close(
        f(session.sensitivities().unwrap()),
        f(fresh.sensitivities().unwrap()),
    );
    assert!((f(old.sensitivities().unwrap()) - f(fresh.sensitivities().unwrap())).abs() > 1e-8);
}

#[test]
fn delta_coil_and_terminal_derivatives_follow_identity_after_reordering() {
    let mut n = initial_tests::case("components", "capacitor_delta");
    n.generators_mut().clear();
    n.buses_mut().reverse();
    n.loads_mut().reverse();
    for bus in n.buses_mut() {
        bus.terminals.reverse();
    }
    let index = n
        .loads()
        .iter()
        .position(|d| d.configuration == powerio_dist::Configuration::Delta)
        .unwrap();
    let id = n.loads()[index].name.clone();
    let mut p = parameter(Owner::Load, &id, Kind::LoadActivePower);
    p.axis = Some(1);
    let r = solve(n.clone());
    let d = r
        .devices
        .iter()
        .find(|d| d.kind == DeviceKind::Load && d.identity == id)
        .unwrap();
    let mut out = vec![Output::Objective];
    for q in [
        Flow::ActivePower,
        Flow::ReactivePower,
        Flow::CurrentReal,
        Flow::CurrentImaginary,
    ] {
        out.push(device(
            DeviceKind::Load,
            &id,
            McOpfDeviceAxis::Coil { index: 1 },
            q,
        ));
        for t in &d.terminals {
            out.push(device(
                DeviceKind::Load,
                &id,
                McOpfDeviceAxis::Terminal {
                    terminal: t.terminal.clone(),
                },
                q,
            ));
        }
    }
    check_fd(n, p, &out, 0.1, |n, d| n.loads_mut()[index].p_nom[1] += d);
}

#[test]
fn physical_derivatives_are_base_objective_scale_and_order_invariant() {
    let mut n = constrained();
    n.lines_mut()[0].i_max = Some(vec![0.2, 20.]);
    let p = parameter(Owner::Branch, "line:l", Kind::CurrentRating);
    let dir = [McOpfParameterDirection {
        parameter: p.clone(),
        delta: 1.,
    }];
    let out = outputs();
    let r = solve(n.clone());
    let base = r.sensitivities().unwrap().forward(&dir, &out).unwrap();
    n.buses_mut().reverse();
    for b in n.buses_mut() {
        b.terminals.reverse();
    }
    for (vb, sb, scale) in [(100., 4000., 0.2), (400., 700., 7.)] {
        let r = solve_options(
            n.clone(),
            McOpfOptions {
                voltage_base_v: vb,
                power_base_va: sb,
                objective_scale: scale,
                collect_duals: true,
                ..Default::default()
            },
        );
        let s = r.sensitivities().unwrap();
        let scaled = s.forward(&dir, &out).unwrap();
        for (a, b) in scaled.iter().zip(&base) {
            close(a.derivative, b.derivative);
        }
        let a = s
            .adjoint(
                &[McOpfOutputWeight {
                    output: Output::Objective,
                    weight: 1.,
                }],
                std::slice::from_ref(&p),
            )
            .unwrap();
        close(a[0].derivative, base[0].derivative);
    }
}

#[test]
fn unsupported_parameters_outputs_and_nonfinite_requests_are_explicit_errors() {
    let r = solve(tests::rich_network());
    let s = r.sensitivities().unwrap();
    for p in [
        parameter(Owner::Generator, "g", Kind::ReactivePowerUpper),
        parameter(Owner::Load, "missing", Kind::LoadActivePower),
    ] {
        let e = s
            .forward(
                &[McOpfParameterDirection {
                    parameter: p,
                    delta: 0.,
                }],
                &[Output::Objective],
            )
            .unwrap_err();
        assert_eq!(e.kind, McOpfSensitivityErrorKind::UnsupportedParameter);
    }
    for output in [
        voltage("s", "n", McOpfVoltagePart::Angle),
        voltage("s", "n", McOpfVoltagePart::Magnitude),
        voltage("missing", "a", McOpfVoltagePart::Real),
        branch(McOpfBranchEnd::From, 99, Flow::ActivePower),
    ] {
        assert_eq!(
            s.forward(&[], &[output]).unwrap_err().kind,
            McOpfSensitivityErrorKind::UnsupportedOutput
        );
    }
    assert_eq!(
        s.forward(
            &[McOpfParameterDirection {
                parameter: load(false),
                delta: f64::NAN
            }],
            &[]
        )
        .unwrap_err()
        .kind,
        McOpfSensitivityErrorKind::InvalidRequest
    );
    assert_eq!(
        s.adjoint(
            &[McOpfOutputWeight {
                output: Output::Objective,
                weight: f64::INFINITY
            }],
            &[]
        )
        .unwrap_err()
        .kind,
        McOpfSensitivityErrorKind::InvalidRequest
    );
    let mut n = tests::network();
    n.loads_mut()[0].voltage_model =
        powerio_dist::DistLoadVoltageModel::ConstantImpedance { v_nom: vec![230.] };
    let r = solve(n);
    let s = r.sensitivities().unwrap();
    let derivatives = s
        .adjoint(
            &[McOpfOutputWeight {
                output: Output::Objective,
                weight: 1.,
            }],
            &[load(false)],
        )
        .unwrap();
    assert!(derivatives[0].derivative.is_finite());
    serde_json::to_string(&derivatives).unwrap();
}

#[test]
fn saved_results_rebuild_sensitivity_views_without_changing_the_report() {
    let r = solve(tests::rich_network());
    let before = r.to_module_json().unwrap();
    let saved = McOpfResult::from_module_json(&before).unwrap();
    let s = saved.sensitivities().unwrap();
    let params = s.parameters();
    let a = s
        .adjoint(
            &[McOpfOutputWeight {
                output: Output::Objective,
                weight: 1.,
            }],
            &params,
        )
        .unwrap();
    for a in &a {
        let m = saved
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == a.parameter)
            .unwrap();
        close(a.derivative, m.derivative.unwrap());
    }
    let dir = [
        McOpfParameterDirection {
            parameter: load(false),
            delta: 0.25,
        },
        McOpfParameterDirection {
            parameter: load(false),
            delta: 0.75,
        },
    ];
    let f = s.forward(&dir, &[Output::Objective]).unwrap();
    close(
        f[0].derivative,
        a.iter()
            .find(|a| a.parameter == load(false))
            .unwrap()
            .derivative,
    );
    assert_eq!(before, saved.to_module_json().unwrap());
}

#[test]
fn nonunique_dispatch_is_not_regularized_into_a_sensitivity() {
    let mut n = tests::rich_network();
    n.lines_mut().clear();
    n.line_codes_mut().clear();
    n.shunts_mut().clear();
    n.buses_mut().truncate(1);
    n.loads_mut()[0].bus = "s".into();
    n.generators_mut()[0].bus = "s".into();
    n.generators_mut()[0].cost = Some(vec![0.2]);
    let r = solve(n);
    let e = r.sensitivities().unwrap_err();
    assert!(
        matches!(
            e.kind,
            McOpfSensitivityErrorKind::SingularKkt | McOpfSensitivityErrorKind::UnreliableDuals
        ),
        "{e:?}"
    );
    assert!(r.residuals.maximum() < 1e-6);
}

#[test]
fn failed_session_attempts_and_option_changes_disable_current_derivatives() {
    let mut s = McOpfSession::new(
        McAcOpfInstance::from_network(tests::network()).unwrap(),
        McOpfOptions {
            collect_duals: true,
            ..Default::default()
        },
    )
    .unwrap();
    s.solve().unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    assert!(s
        .solve_cancellable(McOpfStart::LastAccepted, Some(cancel))
        .is_err());
    assert_eq!(
        s.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
    assert!(s.last_accepted().unwrap().result.sensitivities().is_ok());
    s.solve().unwrap();
    let mut o = s.options().clone();
    o.objective_scale = 2.;
    s.set_options(o).unwrap();
    assert_eq!(
        s.sensitivities().unwrap_err().kind,
        McOpfSensitivityErrorKind::StaleResult
    );
}
