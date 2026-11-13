use powerio::{BalancedNetwork, PioModule, PioValue};

use super::*;
use crate::api::{SensRequest, SolveRequest};
use crate::formulation::AcPolar;
use crate::model::AcNetwork;
use crate::problem::{ac_pf, AcPfSolution};
use crate::sens::{ElementId, Mode, Operand, Parameter, Power, VoltageKind};
use crate::study::{NetworkEdit, Study};
use crate::Problem;

/// Series impedance of the two-bus line (per unit).
const R: f64 = 0.01;
const X: f64 = 0.1;

/// Bus 1 is the slack at `vg`, bus 2 a PQ bus behind an `R + jX` line. Each bus
/// carries the stated load (MW, MVAr) when nonzero.
fn two_bus(slack_load: (f64, f64), load: (f64, f64), base_kv: f64, vg: f64) -> BalancedNetwork {
    let (p1, q1) = slack_load;
    let (p2, q2) = load;
    let text = format!(
        "function mpc = two\nmpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n \
         1 3 {p1} {q1} 0 0 1 {vg} 0 {base_kv} 1 1.1 0.9;\n \
         2 1 {p2} {q2} 0 0 1 1 0 {base_kv} 1 1.1 0.9;\n];\nmpc.gen = [\n \
         1 0 0 300 -300 {vg} 100 1 900 0 0 0 0 0 0 0 0 0 0 0 0;\n];\nmpc.branch = [\n \
         1 2 {R} {X} 0 250 250 250 0 0 1 -360 360;\n];\nmpc.gencost = [\n 2 0 0 3 0 10 0;\n];\n"
    );
    crate::model::parse_matpower(&text).expect("parse two-bus case")
}

/// A meshed four-bus case: slack 1, PV generator at 2 (vg 1.02), PQ buses 3 and 4.
/// Bus 3 carries the stated load and bus shunt (MW / MVAr at 1 pu).
fn four_bus(load3: (f64, f64), shunt3: (f64, f64)) -> BalancedNetwork {
    let (p3, q3) = load3;
    let (gs, bs) = shunt3;
    let text = format!(
        "function mpc = four\nmpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n \
         1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;\n \
         2 2 40 15 0 0 1 1.02 0 230 1 1.1 0.9;\n \
         3 1 {p3} {q3} {gs} {bs} 1 1 0 230 1 1.1 0.9;\n \
         4 1 60 20 0 0 1 1 0 230 1 1.1 0.9;\n];\nmpc.gen = [\n \
         1 0 0 300 -300 1.0 100 1 900 0 0 0 0 0 0 0 0 0 0 0 0;\n \
         2 50 0 200 -200 1.02 100 1 300 0 0 0 0 0 0 0 0 0 0 0 0;\n];\nmpc.branch = [\n \
         1 2 0.01 0.08 0.02 250 250 250 0 0 1 -360 360;\n \
         2 3 0.02 0.10 0.02 250 250 250 0 0 1 -360 360;\n \
         3 4 0.015 0.09 0.01 250 250 250 0 0 1 -360 360;\n \
         4 1 0.01 0.07 0.02 250 250 250 0 0 1 -360 360;\n];\nmpc.gencost = [\n \
         2 0 0 3 0 10 0;\n 2 0 0 3 0 10 0;\n];\n"
    );
    crate::model::parse_matpower(&text).expect("parse four-bus case")
}

/// Give the load at bus `id` the voltage model `model`.
fn with_model(mut net: BalancedNetwork, id: usize, model: LoadVoltageModel) -> BalancedNetwork {
    let load = net
        .loads_mut()
        .iter_mut()
        .find(|load| load.bus.0 == id)
        .expect("a load at the bus");
    load.voltage_model = Some(model);
    net
}

fn solve(net: &BalancedNetwork) -> (AcNetwork, AcPfSolution) {
    let model = AcNetwork::from_network_pf(net).expect("AC power flow model");
    let sol = ac_pf(&AcPolar::new(), &model).expect("AC power flow");
    (model, sol)
}

fn dense(model: &AcNetwork, id: usize) -> usize {
    model.bus_ids.iter().position(|&b| b == id).expect("bus")
}

fn module_json(value: PioValue) -> String {
    crate::ir::serialize_module(&PioModule::new(value)).expect("write module")
}

/// The high-voltage root of the two-bus voltage equation with the slack at 1 pu,
/// `V⁴ + (2(R P + X Q) − 1) V² + (R² + X²)(P² + Q²) = 0`, where the load draws
/// `P(V) + j Q(V)` (per unit). Scans down from 1 pu for the first sign change
/// and bisects it to machine precision: an independent scalar solve.
fn two_bus_voltage(p: &dyn Fn(f64) -> f64, q: &dyn Fn(f64) -> f64) -> f64 {
    let g = |v: f64| {
        let (pv, qv) = (p(v), q(v));
        v.powi(4) + (2.0 * (R * pv + X * qv) - 1.0) * v * v + (R * R + X * X) * (pv * pv + qv * qv)
    };
    let mut hi = 1.0;
    let mut lo = hi - 1e-3;
    while g(lo).signum() == g(hi).signum() {
        hi = lo;
        lo -= 1e-3;
        assert!(lo > 0.05, "no high-voltage root");
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid).signum() == g(hi).signum() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn nominal_voltage_reads_against_the_bus_base() {
    assert_eq!(nominal_pu(None, 0.4, 1).unwrap(), 1.0);
    // 0.4 kV low-voltage bus stating its nominal in kV.
    assert!((nominal_pu(Some(0.4), 0.4, 1).unwrap() - 1.0).abs() < 1e-15);
    assert!((nominal_pu(Some(0.42), 0.4, 1).unwrap() - 1.05).abs() < 1e-12);
    // The same bus stating it in per unit (0.4 pu is implausible, 1.0 kV is 2.5 pu).
    assert_eq!(nominal_pu(Some(1.0), 0.4, 1).unwrap(), 1.0);
    // Transmission bus in kV and in per unit.
    assert!((nominal_pu(Some(230.0), 230.0, 1).unwrap() - 1.0).abs() < 1e-15);
    assert_eq!(nominal_pu(Some(0.98), 230.0, 1).unwrap(), 0.98);
    // A 1 kV bus reads the same either way.
    assert_eq!(nominal_pu(Some(1.0), 1.0, 1).unwrap(), 1.0);
    // A 0.69 kV bus with v_nom = 1 is 1.449 pu in kV or 1 pu in per unit.
    let error = nominal_pu(Some(1.0), 0.69, 7).unwrap_err();
    assert!(
        error.contains("bus 7") && error.contains("unambiguously"),
        "{error}"
    );
    assert!(nominal_pu(Some(11.0), 230.0, 1).is_err());
    assert!(nominal_pu(Some(0.0), 230.0, 1).is_err());
    assert!(nominal_pu(Some(f64::NAN), 230.0, 1).is_err());
    // Unknown base: only the per-unit reading.
    assert_eq!(nominal_pu(Some(1.02), 0.0, 1).unwrap(), 1.02);
    assert!(nominal_pu(Some(11.0), 0.0, 1).is_err());
}

#[test]
fn terms_and_their_derivatives_are_exact_and_finite_near_zero() {
    for &g in &[0.5, 1.0, 1.7, 2.0, 3.0, -1.0] {
        let t = LoadTerm {
            bus: 0,
            reactive: false,
            coefficient: 0.7,
            v_nom: 1.05,
            exponent: g,
        };
        for &vm in &[0.3, 0.97, 1.2] {
            let (f, d) = t.eval(vm);
            assert!((f - 0.7 * (vm / 1.05_f64).powf(g)).abs() < 1e-14);
            let h = 1e-6;
            let fd = (t.eval(vm + h).0 - t.eval(vm - h).0) / (2.0 * h);
            assert!((d - fd).abs() < 1e-8, "γ={g} vm={vm}: {d} vs {fd}");
        }
        // Near zero: finite value and slope, continuous at the floor.
        let (f, d) = t.eval(1e-9);
        assert!(f.is_finite() && d.is_finite());
        let x0 = RATIO_FLOOR * 1.05;
        let below = t.eval(x0 * (1.0 - 1e-12)).0;
        let above = t.eval(x0 * (1.0 + 1e-12)).0;
        assert!((below - above).abs() < 1e-9 * above.abs().max(1.0));
    }
}

/// (i) A ZIP load behind one line solves to the root of the scalar two-bus
/// voltage equation, with the nominal voltage absent, in per unit and in kV.
#[test]
fn two_bus_zip_load_matches_the_closed_form_voltage() {
    for (v_nom, vn) in [(None, 1.0), (Some(1.05), 1.05), (Some(230.0 * 0.98), 0.98)] {
        let net = with_model(
            two_bus((0.0, 0.0), (30.0, 10.0), 230.0, 1.0),
            2,
            zip([12.0, 9.0, 9.0], [4.0, 3.0, 3.0], v_nom),
        );
        let (model, sol) = solve(&net);
        let b = dense(&model, 2);
        let p = |v: f64| {
            let x = v / vn;
            0.12 + 0.09 * x + 0.09 * x * x
        };
        let q = |v: f64| {
            let x = v / vn;
            0.04 + 0.03 * x + 0.03 * x * x
        };
        let v = two_bus_voltage(&p, &q);
        assert!(
            (sol.vm[b] - v).abs() < 1e-9,
            "v_nom {v_nom:?}: {} vs closed form {v}",
            sol.vm[b]
        );
        // The bus injects minus its load at the solved voltage.
        assert!((sol.p[b] + p(v)).abs() < 1e-9);
        assert!((sol.q[b] + q(v)).abs() < 1e-9);
        assert!(model
            .diagnostics
            .iter()
            .any(|d| d.code == "load_voltage_dependent" && d.buses == vec![2]));
    }
}

/// Exponential loads with γ = 0, 1, 2 are constant power, constant current and
/// constant impedance.
#[test]
fn exponential_exponents_zero_one_two_are_constant_power_current_impedance() {
    let base = || two_bus((0.0, 0.0), (80.0, 30.0), 230.0, 1.0);
    let (_, plain) = solve(&base());
    let (_, g0) = solve(&with_model(
        base(),
        2,
        exponential(80.0, 30.0, 0.0, 0.0, None),
    ));
    for (a, b) in plain.vm.iter().zip(&g0.vm) {
        assert!((a - b).abs() < 1e-12);
    }
    for (a, b) in plain.va.iter().zip(&g0.va) {
        assert!((a - b).abs() < 1e-12);
    }
    let (_, g1) = solve(&with_model(
        base(),
        2,
        exponential(80.0, 30.0, 1.0, 1.0, None),
    ));
    let (_, i) = solve(&with_model(
        base(),
        2,
        zip([0.0, 80.0, 0.0], [0.0, 30.0, 0.0], None),
    ));
    assert_eq!(g1.vm, i.vm);
    assert_eq!(g1.va, i.va);
    let (_, g2) = solve(&with_model(
        base(),
        2,
        exponential(80.0, 30.0, 2.0, 2.0, None),
    ));
    let (_, z) = solve(&with_model(
        base(),
        2,
        zip([0.0, 0.0, 80.0], [0.0, 0.0, 30.0], None),
    ));
    assert_eq!(g2.vm, z.vm);
    assert_eq!(g2.va, z.va);
    // The three differ: the voltage dependence is not dropped.
    assert!((plain.vm[1] - g1.vm[1]).abs() > 1e-4);
    assert!((g1.vm[1] - g2.vm[1]).abs() > 1e-4);
}

/// (iii) A constant-impedance load draws `P₂ V² + j Q₂ V²`, which is the bus shunt
/// `gs = P₂`, `bs = −Q₂`: both solve to the same voltages.
#[test]
fn constant_impedance_load_equals_a_shunt_admittance() {
    let (p2, q2) = (35.0, 12.0);
    let (zl, load) = solve(&with_model(
        four_bus((p2, q2), (0.0, 0.0)),
        3,
        zip([0.0, 0.0, p2], [0.0, 0.0, q2], None),
    ));
    let (sh, shunt) = solve(&four_bus((0.0, 0.0), (p2, -q2)));
    assert_eq!(zl.bus_ids, sh.bus_ids);
    for i in 0..zl.n {
        assert!((load.vm[i] - shunt.vm[i]).abs() < 1e-10, "vm[{i}]");
        assert!((load.va[i] - shunt.va[i]).abs() < 1e-10, "va[{i}]");
    }
    assert!((load.vm[dense(&zl, 3)] - 1.0).abs() > 1e-3);
}

/// (iv) Near the nose a fixed point over constant-power solves fails where the
/// exact Newton iteration converges. With γ = 3 the constant-power solve at the
/// last voltage overshoots: the rounds alternate around the operating point and
/// do not settle in 30 rounds. With γ = 2 and a heavier load the first
/// constant-power solve (at 1 pu) has no solution at all.
#[test]
fn newton_converges_near_the_nose_where_the_fixed_point_does_not() {
    // 320 MW / 96 MVAr at 1 pu, γ = 3.
    let net = with_model(
        two_bus((0.0, 0.0), (320.0, 96.0), 230.0, 1.0),
        2,
        exponential(320.0, 96.0, 3.0, 3.0, None),
    );
    let (model, sol) = solve(&net);
    let b = dense(&model, 2);
    let v = two_bus_voltage(&|v| 3.2 * v.powi(3), &|v| 0.96 * v.powi(3));
    assert!((sol.vm[b] - v).abs() < 1e-9, "{} vs {v}", sol.vm[b]);
    assert!(sol.iterations <= 10, "{} iterations", sol.iterations);

    // The former fixed point: the load at the last voltage, solved as constant power.
    let mut vb: f64 = 1.0;
    let mut rounds = Vec::new();
    for _ in 0..30 {
        let mut substitute = model.clone();
        substitute.load_terms.clear();
        substitute.pd[b] = 3.2 * vb.powi(3);
        substitute.qd[b] = 0.96 * vb.powi(3);
        let next = ac_pf(&AcPolar::new(), &substitute).expect("constant-power round");
        vb = next.vm[b];
        rounds.push(vb);
    }
    let last = rounds[rounds.len() - 1];
    let before = rounds[rounds.len() - 2];
    assert!(
        (last - before).abs() > 1e-6,
        "the fixed point settled: {rounds:?}"
    );
    // The rounds alternate around the operating point.
    for pair in rounds[10..].windows(2) {
        assert!((pair[0] - v) * (pair[1] - v) < 0.0, "{rounds:?}");
    }

    // 400 MW / 120 MVAr at 1 pu, γ = 2: no constant-power solution at 1 pu.
    let net = with_model(
        two_bus((0.0, 0.0), (400.0, 120.0), 230.0, 1.0),
        2,
        exponential(400.0, 120.0, 2.0, 2.0, None),
    );
    let (model, sol) = solve(&net);
    let v = two_bus_voltage(&|v| 4.0 * v * v, &|v| 1.2 * v * v);
    assert!((sol.vm[b] - v).abs() < 1e-9, "{} vs {v}", sol.vm[b]);
    let mut substitute = model.clone();
    substitute.load_terms.clear();
    substitute.pd[b] = 4.0;
    substitute.qd[b] = 1.2;
    assert!(ac_pf(&AcPolar::new(), &substitute).is_err());
}

/// (vi) A ZIP load at the slack draws its power at the slack's held voltage, and the
/// slack's net injection balances the losses and the other bus's load at its solved
/// voltage.
#[test]
fn slack_balances_losses_and_loads_at_the_solved_voltage() {
    let vg = 1.03;
    let net = with_model(
        with_model(
            two_bus((50.0, 20.0), (60.0, 25.0), 230.0, vg),
            1,
            zip([20.0, 15.0, 15.0], [8.0, 6.0, 6.0], None),
        ),
        2,
        exponential(60.0, 25.0, 1.5, 2.5, None),
    );
    let (model, sol) = solve(&net);
    let (s, b) = (dense(&model, 1), dense(&model, 2));
    assert_eq!(model.slacks, vec![s]);
    assert!((sol.vm[s] - vg).abs() < 1e-15);
    let slack_load = (
        0.20 + 0.15 * vg + 0.15 * vg * vg,
        0.08 + 0.06 * vg + 0.06 * vg * vg,
    );
    let (pd, qd) = model.demand_at(&sol.vm);
    assert!((pd[s] - slack_load.0).abs() < 1e-15);
    assert!((qd[s] - slack_load.1).abs() < 1e-15);
    let v2 = sol.vm[b];
    let load = (0.6 * v2.powf(1.5), 0.25 * v2.powf(2.5));
    let [pf, qf, pt, qt] = crate::emit::ac_terminal_flows(&model, &sol);
    let (p_loss, q_loss) = (pf[0] + pt[0], qf[0] + qt[0]);
    // The slack's generation is its net injection plus its own load.
    let p_gen = sol.p[s] + pd[s];
    let q_gen = sol.q[s] + qd[s];
    assert!((p_gen - (p_loss + slack_load.0 + load.0)).abs() < 1e-9);
    assert!((q_gen - (q_loss + slack_load.1 + load.1)).abs() < 1e-9);
}

/// (ix) A 0.4 kV low-voltage bus whose load states its nominal voltage in kV: 0.4 kV
/// is the bus base (1 pu) and 0.38 kV is 0.95 pu, not 0.38 pu.
#[test]
fn low_voltage_nominal_in_kilovolts_reads_against_the_bus_base() {
    let base = || two_bus((0.0, 0.0), (0.05, 0.02), 0.4, 1.0);
    let gamma = 1.5;
    let (_, none) = solve(&with_model(
        base(),
        2,
        exponential(0.05, 0.02, gamma, gamma, None),
    ));
    let (_, kv) = solve(&with_model(
        base(),
        2,
        exponential(0.05, 0.02, gamma, gamma, Some(0.4)),
    ));
    assert_eq!(none.vm, kv.vm);
    assert_eq!(none.va, kv.va);

    let (model, low) = solve(&with_model(
        base(),
        2,
        exponential(0.05, 0.02, gamma, gamma, Some(0.38)),
    ));
    assert!(model
        .load_terms
        .iter()
        .all(|t| (t.v_nom - 0.95).abs() < 1e-15));
    let scale = 0.95_f64.powf(-gamma);
    let (_, scaled) = solve(&with_model(
        base(),
        2,
        exponential(0.05 * scale, 0.02 * scale, gamma, gamma, None),
    ));
    for (a, b) in low.vm.iter().zip(&scaled.vm) {
        assert!((a - b).abs() < 1e-12);
    }
}

/// (vii) The Study's AC voltage sensitivity to active demand on a case with ZIP and
/// exponential loads matches central differences of committed demand edits: the
/// sensitivity differentiates the solved system, load voltage dependence included.
#[test]
fn study_voltage_sensitivity_matches_central_differences_with_zip_loads() {
    let net = with_model(
        with_model(
            four_bus((70.0, 25.0), (0.0, 0.0)),
            3,
            zip([20.0, 25.0, 25.0], [5.0, 10.0, 10.0], None),
        ),
        4,
        exponential(60.0, 20.0, 1.7, 3.0, None),
    );
    let json = module_json(PioValue::BalancedNetwork(net));
    let mut study = Study::new(&json, Problem::AcPf).expect("study");
    let vm_op = Operand::Voltage(VoltageKind::Magnitude);
    let request = SensRequest {
        operand: vm_op,
        parameter: Parameter::Demand(Power::Active),
        indices: None,
        mode: Mode::Forward,
    };
    let response = study.commit_with(&[], &[request]).expect("sensitivities");
    let matrix = &response.sensitivities[0];
    let h = 1e-3; // MW
    let vm_after = |mw: f64, bus: usize| -> Vec<(usize, f64)> {
        let mut fork = study.fork();
        let r = fork
            .commit(&[NetworkEdit::AddLoad {
                bus: (bus as i64).into(),
                p_mw: mw,
            }])
            .expect("edit");
        r.vm.unwrap().iter().map(|v| (v.bus, v.value)).collect()
    };
    let mut checked = 0;
    for (c, col) in matrix.cols.iter().enumerate() {
        let ElementId::Bus(bus) = col.element else {
            panic!("bus column")
        };
        if bus == 1 {
            continue; // the slack absorbs it
        }
        let plus = vm_after(h, bus);
        let minus = vm_after(-h, bus);
        for (r, row) in matrix.rows.iter().enumerate() {
            let ElementId::Bus(at) = row.element else {
                panic!("bus row")
            };
            let p = plus.iter().find(|(b, _)| *b == at).unwrap().1;
            let m = minus.iter().find(|(b, _)| *b == at).unwrap().1;
            let fd = (p - m) / (2.0 * h);
            let an = matrix.values[r][c];
            assert!(
                (an - fd).abs() < 1e-7 + 1e-5 * fd.abs(),
                "dvm[{at}]/dP[{bus}]: analytic {an}, finite difference {fd}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 4);
    // Not vacuous: the derivatives are well above the tolerance. Dropping the load
    // derivative from the sensitivity Jacobian misses the finite difference by ~4%.
    assert!(matrix.values.iter().flatten().any(|v| v.abs() > 1e-4));
}

/// (viii) A Study over an AC power flow instance accepts ZIP loads and solves the
/// same operating point as the network Study and the one-shot solve.
#[test]
fn ac_pf_instance_study_accepts_voltage_dependent_loads() {
    let net = with_model(
        four_bus((70.0, 25.0), (0.0, 0.0)),
        3,
        zip([20.0, 25.0, 25.0], [5.0, 10.0, 10.0], None),
    );
    let instance = powerio::AcPfInstance::from_network(net.clone()).expect("instance");
    let from_instance = Study::new(
        &module_json(PioValue::AcPfInstance(instance)),
        Problem::AcPf,
    )
    .expect("instance study");
    let from_network = Study::new(
        &module_json(PioValue::BalancedNetwork(net.clone())),
        Problem::AcPf,
    )
    .expect("network study");
    let req = SolveRequest {
        formulation: Problem::AcPf,
        ..SolveRequest::default()
    };
    let one_shot = crate::api::solve_network(&net, &req).expect("one-shot");
    let (model, sol) = solve(&net);
    let vm = |r: &crate::api::SolveResponse| -> Vec<f64> {
        r.vm.as_ref().unwrap().iter().map(|v| v.value).collect()
    };
    let a = vm(from_instance.solution());
    let b = vm(from_network.solution());
    let c = vm(&one_shot);
    for i in 0..model.n {
        assert!((a[i] - sol.vm[i]).abs() < 1e-12);
        assert!((b[i] - sol.vm[i]).abs() < 1e-12);
        assert!((c[i] - sol.vm[i]).abs() < 1e-12);
    }
    for response in [from_instance.solution(), from_network.solution(), &one_shot] {
        assert!(response
            .diagnostics
            .iter()
            .any(|d| d.code == "load_voltage_dependent"));
    }
}

/// SOCWR and the other constant-power consumers refuse voltage-dependent loads and
/// name the AC power flow that solves them.
#[cfg(feature = "conic")]
#[test]
fn socwr_refuses_voltage_dependent_loads_clearly() {
    let net = with_model(
        four_bus((70.0, 25.0), (0.0, 0.0)),
        3,
        zip([20.0, 25.0, 25.0], [5.0, 10.0, 10.0], None),
    );
    let Err(error) = Study::new(&module_json(PioValue::BalancedNetwork(net)), Problem::Socwr)
    else {
        panic!("SOCWR refuses ZIP loads");
    };
    assert!(
        error.contains("SOCWR") && error.contains("constant-power") && error.contains("acpf"),
        "{error}"
    );
}

/// A voltage-dependent load whose nominal voltage reads two ways is refused.
#[test]
fn an_ambiguous_nominal_voltage_is_refused() {
    let net = with_model(
        two_bus((0.0, 0.0), (0.05, 0.02), 0.69, 1.0),
        2,
        exponential(0.05, 0.02, 1.0, 2.0, Some(1.0)),
    );
    let error = AcNetwork::from_network_pf(&net).unwrap_err();
    assert!(error.contains("unambiguously"), "{error}");
}
