//! Physical acceptance calculations. Does not evaluate the expression graph.
use super::{
    model::{Model, Pair},
    McOpfBranchResult, McOpfDeviceResult, McOpfResiduals,
};
use num_complex::Complex64 as C;
use powerio_matrix::McOpfDeviceKind;
pub(super) struct Checked {
    pub voltage: Vec<C>,
    pub residuals: McOpfResiduals,
    pub branches: Vec<McOpfBranchResult>,
    pub devices: Vec<McOpfDeviceResult>,
    pub objective: f64,
}
fn value(p: &Pair, x: &[f64]) -> C {
    C::new(p[0].value(x), p[1].value(x))
}
fn array(v: C, base: f64) -> [f64; 2] {
    [v.re * base, v.im * base]
}
fn mismatch(target: &mut f64, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err("nonfinite independently recomputed residual".into());
    }
    *target = target.max(value.abs());
    Ok(())
}
fn cap(target: &mut f64, value: f64, limit: Option<f64>) -> Result<(), String> {
    if let Some(limit) = limit {
        mismatch(
            target,
            if limit == 0.0 {
                value.max(0.0)
            } else {
                (value / limit - 1.0).max(0.0)
            },
        )?;
    }
    Ok(())
}
fn interval(target: &mut f64, value: f64, lo: Option<f64>, hi: Option<f64>) -> Result<(), String> {
    if let Some(lo) = lo {
        mismatch(target, (lo - value).max(0.0) / lo.abs().max(1.0))?;
    }
    if let Some(hi) = hi {
        mismatch(target, (value - hi).max(0.0) / hi.abs().max(1.0))?;
    }
    Ok(())
}
pub(super) fn check(
    model: &Model,
    x: &[f64],
    solver_objective: f64,
    tolerance: f64,
) -> Result<Checked, String> {
    if x.len() != model.problem.n
        || x.iter().any(|x| !x.is_finite())
        || !solver_objective.is_finite()
    {
        return Err("invalid or nonfinite solver point/objective".into());
    }
    let prep = &model.prep;
    let sb = prep.bases.power_base_va;
    let ib = sb / prep.bases.voltage_base_v;
    let voltage: Vec<_> = model.voltage.iter().map(|v| value(v, x)).collect();
    let mut kcl = vec![C::new(0.0, 0.0); voltage.len()];
    let mut residuals = McOpfResiduals::default();
    let mut branches = Vec::new();
    for ((br, cols), power_columns) in prep
        .branches
        .iter()
        .zip(&model.branch_current)
        .zip(&model.branch_power)
    {
        let currents: Vec<_> = cols.iter().map(|i| value(i, x)).collect();
        let mut ends: [Vec<C>; 2] = [Vec::new(), Vec::new()];
        let mut powers: [Vec<C>; 2] = [Vec::new(), Vec::new()];
        for k in 0..br.from.len() {
            if !br.open {
                let drop: C = currents
                    .iter()
                    .enumerate()
                    .map(|(j, i)| C::new(br.r[k][j], br.x[k][j]) * i)
                    .sum();
                mismatch(
                    &mut residuals.kvl_pu,
                    (voltage[br.from[k]] - voltage[br.to[k]] - drop).norm(),
                )?;
            }
            for (end, nodes, g, b, sign) in [
                (0, &br.from, &br.g_from, &br.b_from, 1.0),
                (1, &br.to, &br.g_to, &br.b_to, -1.0),
            ] {
                let shunt: C = nodes
                    .iter()
                    .enumerate()
                    .map(|(j, &node)| C::new(g[k][j], b[k][j]) * voltage[node])
                    .sum();
                let current = if br.open {
                    C::new(0.0, 0.0)
                } else {
                    sign * currents[k] + shunt
                };
                let p = voltage[nodes[k]] * current.conj();
                kcl[nodes[k]] -= current;
                cap(
                    &mut residuals.relative_limit_violation,
                    current.norm(),
                    br.current_max[k],
                )?;
                cap(
                    &mut residuals.relative_limit_violation,
                    p.norm(),
                    br.apparent_max[k],
                )?;
                if let Some(indices) = power_columns[k][end] {
                    for (index, physical) in indices.into_iter().zip([p.re, p.im]) {
                        mismatch(&mut residuals.power_link_pu, x[index] - physical)?;
                    }
                }
                ends[end].push(current);
                powers[end].push(p);
            }
        }
        branches.push(McOpfBranchResult {
            identity: br.identity.clone(),
            current_from_a: ends[0].iter().map(|v| array(*v, ib)).collect(),
            current_to_a: ends[1].iter().map(|v| array(*v, ib)).collect(),
            power_from_va: powers[0].iter().map(|v| array(*v, sb)).collect(),
            power_to_va: powers[1].iter().map(|v| array(*v, sb)).collect(),
        });
    }
    for sh in &prep.shunts {
        for (k, &node) in sh.terminals.iter().enumerate() {
            kcl[node] -= sh
                .terminals
                .iter()
                .enumerate()
                .map(|(j, &n)| C::new(sh.g[k][j], sh.b[k][j]) * voltage[n])
                .sum::<C>();
        }
    }
    let mut devices = Vec::new();
    let mut objective = 0.0;
    for (dev, cols) in prep.devices.iter().zip(&model.devices) {
        let mut coil_power = Vec::new();
        let mut coil_current = Vec::new();
        let mut terminal_current = vec![C::new(0.0, 0.0); dev.terminals.len()];
        let mut total = C::new(0.0, 0.0);
        let sign = if dev.kind == McOpfDeviceKind::Load {
            -1.0
        } else {
            1.0
        };
        for (k, coil) in dev.coils.iter().enumerate() {
            let current = value(&cols.current[k], x);
            let u = voltage[coil.positive] - coil.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]);
            let p = u * current.conj();
            mismatch(
                &mut residuals.power_link_pu,
                (p - C::new(x[cols.power[k][0]], x[cols.power[k][1]])).norm(),
            )?;
            if let Some(target) = coil.prescribed {
                mismatch(
                    &mut residuals.prescribed_power_pu,
                    (p - C::new(target[0], target[1])).norm(),
                )?;
            }
            interval(
                &mut residuals.relative_limit_violation,
                p.re,
                coil.p_min,
                coil.p_max,
            )?;
            interval(
                &mut residuals.relative_limit_violation,
                p.im,
                coil.q_min,
                coil.q_max,
            )?;
            cap(
                &mut residuals.relative_limit_violation,
                current.norm(),
                coil.current_max,
            )?;
            cap(
                &mut residuals.relative_limit_violation,
                p.norm(),
                coil.apparent_max,
            )?;
            objective += coil.cost * p.re;
            kcl[coil.positive] += sign * current;
            let pos = dev
                .terminals
                .iter()
                .position(|i| *i == coil.positive)
                .ok_or("missing device positive terminal")?;
            terminal_current[pos] += current;
            if let Some(n) = coil.negative {
                kcl[n] -= sign * current;
                let neg = dev
                    .terminals
                    .iter()
                    .position(|i| *i == n)
                    .ok_or("missing device negative terminal")?;
                terminal_current[neg] -= current;
            }
            total += current;
            coil_power.push(array(p, sb));
            coil_current.push(array(current, ib));
        }
        cap(
            &mut residuals.relative_limit_violation,
            total.norm(),
            dev.neutral_current_max,
        )?;
        devices.push(McOpfDeviceResult {
            identity: dev.identity.clone(),
            kind: dev.kind,
            coil_power_va: coil_power,
            coil_current_a: coil_current,
            terminal_power_va: dev
                .terminals
                .iter()
                .zip(terminal_current)
                .map(|(&n, i)| array(voltage[n] * i.conj(), sb))
                .collect(),
        });
    }
    for limit in &prep.voltage_limits {
        let v = (voltage[limit.positive] - limit.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]))
            .norm();
        cap(&mut residuals.relative_limit_violation, v, limit.upper)?;
        if let Some(floor) = limit.lower.filter(|x| *x > 0.0) {
            mismatch(
                &mut residuals.relative_limit_violation,
                (1.0 - v / floor).max(0.0),
            )?;
        }
    }
    for (i, t) in prep.terminals.iter().enumerate() {
        if !t.grounded {
            mismatch(&mut residuals.kcl_pu, kcl[i].norm())?;
        }
    }
    mismatch(
        &mut residuals.objective_relative_error,
        (objective - solver_objective) / objective.abs().max(1.0),
    )?;
    if residuals.maximum() > tolerance {
        return Err(format!(
            "independent multiconductor OPF validation failed: {residuals:?}"
        ));
    }
    Ok(Checked {
        voltage,
        residuals,
        branches,
        devices,
        objective,
    })
}
