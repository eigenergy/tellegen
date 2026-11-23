use super::*;
use powerio::{BalancedNetwork, BranchCharging, Source};
use powerio_prob::{
    AcOtsOptions, BranchCurrentLimit, BranchSelection, BranchSwitchingCost, CurrentLimitPolicy,
};

fn network() -> BalancedNetwork {
    let raw = include_bytes!("../../tests/data/dc_variants/case3_variants.m");
    let module = powerio::parse(Source::from_memory("case3.m", raw.as_slice()).unwrap()).unwrap();
    crate::ir::balanced_module(module).unwrap().value().clone()
}

fn instance(net: BalancedNetwork, options: AcOtsOptions) -> Arc<AcOtsInstance> {
    let base = AcOpfInstance::from_network(net).unwrap();
    Arc::new(
        AcOtsInstance::new(base, options)
            .unwrap()
            .with_switching_objective(),
    )
}

#[test]
fn line_ends_match_directional_derivatives_of_the_nonlinear_pi_model() {
    let mut net = network();
    net.shunts_mut()
        .push(powerio::Shunt::new(powerio::BusId(2), 1.3, -2.1));
    net.branches_mut()[0].charging = Some(BranchCharging::new(0.002, 0.013, 0.003, 0.024));
    let mut options = AcOtsOptions::default();
    options.max_switched = Some(0);
    let ots = instance(net, options);
    let plan = solve_ots(ots.clone(), OtsModel::Lacots, 1)
        .unwrap()
        .best
        .unwrap();
    let d = plan.dispatch;
    let (fixed, _) = ots.fix_topology(&plan.closed).unwrap();
    let p = build_ac_opf_preparation(&fixed, &Default::default()).unwrap();
    let n = p.n_buses;
    let at = |step: f64| {
        let v: Vec<_> = d
            .vm_pu
            .as_ref()
            .unwrap()
            .iter()
            .map(|vm| 1.0 + step * (vm - 1.0))
            .collect();
        let a: Vec<_> = d.va_rad.iter().map(|a| step * a).collect();
        p.branches.calc_branch_flows(&v, &a)
    };
    let h = 1e-4;
    let (base, plus, minus) = (at(0.0), at(h), at(-h));
    assert!(base.pf[0].abs() > 0.01 && base.qf[0].abs() > 0.01);
    for (affine, flat, plus, minus) in [
        (&d.p_from_pu, base.pf, plus.pf, minus.pf),
        (d.q_from_pu.as_ref().unwrap(), base.qf, plus.qf, minus.qf),
        (&d.p_to_pu, base.pt, plus.pt, minus.pt),
        (d.q_to_pu.as_ref().unwrap(), base.qt, plus.qt, minus.qt),
    ] {
        for e in 0..p.n_branches() {
            assert!((affine[e] - flat[e] - (plus[e] - minus[e]) / (2.0 * h)).abs() < 1e-8);
        }
    }
    let vm = d.vm_pu.unwrap();
    let mut active = powerio_matrix::calc_bus_totals(n, &p.generators.bus_of_gen, &d.pg_pu);
    let mut reactive =
        powerio_matrix::calc_bus_totals(n, &p.generators.bus_of_gen, &d.qg_pu.unwrap());
    let (qf, qt) = (d.q_from_pu.unwrap(), d.q_to_pu.unwrap());
    for e in 0..p.n_branches() {
        let (f, t) = (p.branches.from_bus[e], p.branches.to_bus[e]);
        active[f] -= d.p_from_pu[e];
        active[t] -= d.p_to_pu[e];
        reactive[f] -= qf[e];
        reactive[t] -= qt[e];
        assert!(d.p_from_pu[e].hypot(qf[e]) <= p.branches.s_max[e] + 1e-7);
        assert!(d.p_to_pu[e].hypot(qt[e]) <= p.branches.s_max[e] + 1e-7);
    }
    for b in 0..n {
        let squared = 2.0 * vm[b] - 1.0;
        assert!((active[b] - p.buses.p_d[b] - p.buses.g_s[b] * squared).abs() < 1e-7);
        assert!((reactive[b] - p.buses.q_d[b] + p.buses.b_s[b] * squared).abs() < 1e-7);
    }
}

#[test]
fn switched_plans_restore_to_the_same_ac_problem_as_the_reference() {
    let mut options = AcOtsOptions::default();
    options.max_switched = Some(1);
    let ots = instance(network(), options);
    let ac = solve_ots(ots.clone(), OtsModel::Acots, 4).unwrap();
    assert!(ac.exhausted);
    let reference = ac.best.unwrap();
    assert_eq!(reference.closed, [false, true, true]);
    assert!((reference.objective - 641.00212).abs() < 1e-3);
    for model in [OtsModel::Dcots, OtsModel::Lacots] {
        let search = solve_ots(ots.clone(), model, 4).unwrap();
        assert!(search.exhausted);
        assert_eq!(search.solved + search.infeasible, 4);
        let plan = search.best.unwrap();
        assert_eq!(plan.closed, reference.closed);
        assert!((plan.objective - 622.0).abs() < 1e-4);
        let r = restore_ac(ots.clone(), &plan).unwrap();
        assert_eq!(r.solution.branch_closed(), plan.closed);
        assert_eq!(r.solution.branches_switched(), 1);
        assert!((r.solution.objective() - reference.objective).abs() < 1e-4);
        assert!(r.initial_p_mismatch_mw > 1.0);
        assert!(r.redispatch_l1_mw > 1.0);
        assert!(r.solution.residuals().max_active_power_mismatch.unwrap() < 1e-5);
        assert!(r.solution.residuals().max_reactive_power_mismatch.unwrap() < 1e-5);
    }
    let capped = solve_ots(ots, OtsModel::Lacots, 1).unwrap();
    assert!(!capped.exhausted && capped.best.unwrap().closed.iter().all(|s| *s));
}

#[test]
fn initial_open_lines_can_close_and_switching_cost_is_counted_once() {
    let mut net = network();
    net.branches_mut()[1].in_service = false;
    let id = AcOtsInstance::from_network(net.clone()).unwrap().branches()[1]
        .id
        .clone();
    let mut opts = AcOtsOptions::default();
    opts.switchable = BranchSelection::Only(vec![id.clone()]);
    opts.max_open = Some(0);
    opts.switching_costs
        .push(BranchSwitchingCost::new(id, 17.0, 43.0));
    let ots = instance(net, opts);
    let base = solve_ots(
        instance(network(), AcOtsOptions::default()),
        OtsModel::Lacots,
        1,
    )
    .unwrap()
    .best
    .unwrap();
    let search = solve_ots(ots.clone(), OtsModel::Lacots, 2).unwrap();
    assert!(search.exhausted);
    assert_eq!(search.policy_rejected, 1);
    let plan = search.best.unwrap();
    assert_eq!(plan.closed, [true; 3]);
    assert!((plan.objective - base.objective - 43.0).abs() < 1e-6);
    let r = restore_ac(ots, &plan).unwrap();
    assert!((r.solution.objective() - r.solution.ac_solution().objective() - 43.0).abs() < 1e-8);
}

#[test]
fn reactive_inadequacy_survives_dc_planning_and_fails_ac_restoration() {
    let mut net = network();
    for g in net.generators_mut() {
        g.qmax = 0.0;
        g.qmin = 0.0;
    }
    let mut opts = AcOtsOptions::default();
    opts.switchable = BranchSelection::None;
    let ots = instance(net, opts);
    let dc = solve_ots(ots.clone(), OtsModel::Dcots, 1)
        .unwrap()
        .best
        .unwrap();
    let lac = solve_ots(ots.clone(), OtsModel::Lacots, 1).unwrap();
    assert!(lac.best.is_none());
    assert_eq!(lac.infeasible, 1);
    assert!(restore_ac(ots, &dc)
        .unwrap_err()
        .contains("AC restoration failed"));
}

#[test]
fn current_rating_remains_active_during_ac_redispatch() {
    let net = network();
    let id = AcOtsInstance::from_network(net.clone()).unwrap().branches()[1]
        .id
        .clone();
    let mut opts = AcOtsOptions::default();
    opts.current_limits = CurrentLimitPolicy::Stated;
    opts.stated_current_limits
        .push(BranchCurrentLimit::new(id, None, Some(0.12)));
    opts.switchable = BranchSelection::None;
    let ots = instance(net, opts);
    let lac = solve_ots(ots.clone(), OtsModel::Lacots, 1)
        .unwrap()
        .best
        .unwrap();
    let r = restore_ac(ots, &lac).unwrap();
    let ac = r.solution.ac_solution();
    let s = ac.branch_to_active_flows()[1].hypot(ac.branch_to_reactive_flows()[1]);
    assert!((s / (100.0 * ac.bus_voltage_magnitudes()[2]) - 0.12).abs() < 1e-5);
}
