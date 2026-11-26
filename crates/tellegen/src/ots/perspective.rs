//! Flat-start coefficients for the connected, on/off perspective formulation.
use num_complex::Complex64;
use powerio_matrix::{build_ac_opf_preparation, AnalysisBranchSource};
use powerio_prob::{AcOtsInstance, IslandingPolicy, ObjectiveTerm};
use serde_json::{json, Value};

use super::check_fixed;
use crate::model::PiecewiseCost;

/// Prepare the two terminal Taylor expansions using the same derivatives and
/// per-unit costs as fixed-topology LACOTS. Each affine map takes
/// [1, |V_f|-1, |V_t|-1, theta_f-theta_t] with angles in radians.
pub fn lacots_perspective_data(ots: &AcOtsInstance) -> Result<Value, String> {
    let initial = ots.initial_closed();
    if initial.contains(&false) || ots.options().islanding != IslandingPolicy::Forbidden {
        return Err(
            "perspective LACOTS requires initially closed lines and forbidden islanding".into(),
        );
    }
    let (fixed, _) = ots.fix_topology(&initial).map_err(|e| e.to_string())?;
    check_fixed(ots, &fixed)?;
    let p = build_ac_opf_preparation(&fixed, &Default::default()).map_err(|e| e.to_string())?;
    let br = &p.branches;
    if p.reference_buses.len() != 1 || br.len() != ots.branches().len() {
        return Err(
            "perspective LACOTS requires one connected island and physical two-terminal branches"
                .into(),
        );
    }
    let b = &p.buses;
    let mut buses = Vec::new();
    for i in 0..p.n_buses {
        if !b.voltage_bound_active[i] || !b.vm_min[i].is_finite() || !b.vm_max[i].is_finite() {
            return Err("perspective LACOTS requires finite voltage magnitude bounds".into());
        }
        buses.push(json!({"h":[b.vm_min[i]-1., b.vm_max[i]-1.], "pd":b.p_d[i],
            "qd":b.q_d[i], "gs":b.g_s[i], "bs":b.b_s[i]}));
    }
    let g = &p.generators;
    let mut generators = Vec::new();
    for i in 0..p.n_generators() {
        let curve = g.piecewise_linear[i]
            .clone()
            .map(PiecewiseCost::from_prepared);
        if curve.is_none() && g.q[i] < 0. {
            return Err("LACOTS requires convex generation costs".into());
        }
        generators.push(json!({"bus":g.bus_of_gen[i], "p":g.capability_active[i].then_some([g.pmin[i],g.pmax[i]]),
            "q":g.capability_active[i].then_some([g.qmin[i],g.qmax[i]]),
            "quadratic":0.5*g.q[i], "linear":g.c[i], "constant":g.c0[i],
            "segments":curve.map(|c| c.slopes.into_iter().zip(c.intercepts).collect::<Vec<_>>())}));
    }
    let vm = vec![1.; p.n_buses];
    let va = vec![0.; p.n_buses];
    let flat = br.calc_branch_flows(&vm, &va);
    let switching = ots
        .objective()
        .contains(&ObjectiveTerm::BranchSwitchingCost);
    let mut branches = Vec::new();
    for (e, row) in ots.branches().iter().enumerate() {
        if br.analysis_sources[e] != (AnalysisBranchSource::Branch { row: e }) {
            return Err(
                "perspective LACOTS requires unlowered physical branches in source order".into(),
            );
        }
        let (df, dt) = br.calc_branch_flow_partials(e, &vm, &va);
        let terminal = |pp: f64, qq: f64, d: [Complex64; 4]| {
            [
                [pp, d[1].re, d[3].re, d[0].re],
                [qq, d[1].im, d[3].im, d[0].im],
            ]
        };
        branches.push(
            json!({"id":row.id, "buses":[br.from_bus[e],br.to_bus[e]], "switchable":row.switchable,
            "cost":if switching {row.switching_cost.map_or(0.,|c| c.open)} else {0.},
            "ends":[terminal(flat.pf[e],flat.qf[e],df),terminal(flat.pt[e],flat.qt[e],dt)],
            "angle":br.angle_bound_active[e].then_some([br.angle_min[e],br.angle_max[e]]),
            "rating":br.thermal_limit_active[e].then_some(br.s_max[e]),
            "current":[row.current.from_pu,row.current.to_pu]}),
        );
    }
    Ok(
        json!({"root":p.reference_buses.iter().next(), "buses":buses, "generators":generators,
        "branches":branches, "max_open":ots.options().max_open, "max_switched":ots.options().max_switched}),
    )
}
