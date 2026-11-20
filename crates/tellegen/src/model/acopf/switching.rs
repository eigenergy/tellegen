//! Recover a dispatch on the fixed topology; PowerIO keeps terminal current
//! limits on the OTS instance, so they must be added back to the AC OPF.
use super::*;
use crate::ots::{current_bounds, Dispatch};
use powerio_prob::AcOtsInstance;

pub(crate) fn solve_ots_dispatch(
    ots: &AcOtsInstance,
    fixed: &AcOpfInstance,
    seed: Option<&Dispatch>,
) -> Result<AcOpfSolved, String> {
    let model = compile_ac_opf_model_with(fixed, |p, col, _, parts| {
        if let Some(d) = seed {
            for i in 0..p.n_buses {
                parts.x0[col.va[i]] = d.va_rad[i];
                let vm = col.vm[i];
                parts.x0[vm] = d
                    .vm_pu
                    .as_ref()
                    .map_or(1.0, |v| v[i])
                    .clamp(parts.x_l[vm], parts.x_u[vm]);
            }
            for g in 0..p.n_generators() {
                let (pg, qg) = (col.pg[g], col.qg[g]);
                parts.x0[pg] = d.pg_pu[g].clamp(parts.x_l[pg], parts.x_u[pg]);
                if let Some(q) = &d.qg_pu {
                    parts.x0[qg] = q[g].clamp(parts.x_l[qg], parts.x_u[qg]);
                }
                if let (Some(c), Some(cost)) =
                    (col.piecewise_epigraph[g], &p.generators.piecewise_linear[g])
                {
                    parts.x0[c] = piecewise_value(cost, parts.x0[pg]);
                }
            }
        }
        let v2: Vec<_> = col.vm.iter().map(|&c| cse(square(variable(c)))).collect();
        for e in 0..p.n_branches() {
            let i = current_bounds(ots, p, e);
            let zero_mva = p.branches.thermal_limit_active[e] && p.branches.s_max[e] == 0.0;
            if !zero_mva && i.from_pu.is_none() && i.to_pu.is_none() {
                continue;
            }
            let s = branch_flow(p, col, &v2, e);
            let ends = [
                (
                    "from",
                    p.branches.from_bus[e],
                    s.p_from,
                    s.q_from,
                    i.from_pu,
                ),
                ("to", p.branches.to_bus[e], s.p_to, s.q_to, i.to_pu),
            ];
            for (end, bus, active, reactive, imax) in ends {
                let name = &p.branches.identities[e];
                if zero_mva {
                    // A zero-radius squared thermal row has zero gradient at its solution.
                    hybrid::row(parts, format!("zero_p_{end}[{name}]"), active, 0.0, 0.0);
                    hybrid::row(parts, format!("zero_q_{end}[{name}]"), reactive, 0.0, 0.0);
                } else if let Some(imax) = imax {
                    let margin = subtract(
                        scaled(imax.powi(2), v2[bus].clone()),
                        add(square(active), square(reactive)),
                    );
                    hybrid::row(parts, format!("current_{end}[{name}]"), margin, 0.0, INF);
                }
            }
        }
        Ok(())
    })?;
    let solved = solve_compiled(model, None, POUNCE_OPTIONS, |_, _| Ok(()))?;
    // Re-evaluate |S| <= Imax |V| and finite zero ratings in physical units,
    // independently of the NLP status and squared constraint scaling.
    let p = &solved.preparation;
    for e in 0..p.n_branches() {
        let i = current_bounds(ots, p, e);
        let ends = [
            (
                p.branches.from_bus[e],
                solved.p_from[e].hypot(solved.q_from[e]),
                i.from_pu,
            ),
            (
                p.branches.to_bus[e],
                solved.p_to[e].hypot(solved.q_to[e]),
                i.to_pu,
            ),
        ];
        for (bus, flow, imax) in ends {
            let mva = if p.branches.thermal_limit_active[e] {
                p.branches.s_max[e]
            } else {
                f64::INFINITY
            };
            let limit = imax.map_or(mva, |i| mva.min(i * solved.vm[bus]));
            if flow > limit + PRIMAL_TOLERANCE {
                return Err(format!(
                    "branch {} terminal power {flow} exceeds {limit} pu",
                    p.branches.identities[e]
                ));
            }
        }
    }
    Ok(solved)
}
