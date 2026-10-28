//! Physical acceptance calculations. Does not evaluate the expression graph.
use super::{
    model::{Model, Pair},
    McOpfBranchResult, McOpfCoilAxis, McOpfDeviceResult, McOpfError, McOpfErrorKind,
    McOpfPortResult, McOpfResiduals, McOpfShuntResult, McOpfStage, McOpfTerminalId, McOpfViolation,
};
use num_complex::Complex64 as C;
use powerio_matrix::McOpfDeviceKind;
pub(super) struct Checked {
    pub voltage: Vec<C>,
    pub shunts: Vec<McOpfShuntResult>,
    pub residuals: McOpfResiduals,
    pub branches: Vec<McOpfBranchResult>,
    pub devices: Vec<McOpfDeviceResult>,
    pub transformers: Vec<super::McOpfTransformerResult>,
    pub objective: f64,
}
fn value(p: &Pair, x: &[f64]) -> C {
    C::new(p[0].value(x), p[1].value(x))
}
fn array(v: C, base: f64) -> [f64; 2] {
    [v.re * base, v.im * base]
}
struct Checks {
    residuals: McOpfResiduals,
    tolerance: f64,
    component: String,
    axis: Option<String>,
    violations: std::collections::BTreeMap<&'static str, McOpfViolation>,
}
impl Checks {
    fn new(tolerance: f64) -> Self {
        Self {
            residuals: McOpfResiduals::default(),
            tolerance,
            component: "instance".into(),
            axis: None,
            violations: Default::default(),
        }
    }
    fn at(&mut self, component: impl Into<String>, axis: Option<String>) {
        self.component = component.into();
        self.axis = axis;
    }
    fn failure(&self, message: impl Into<String>) -> McOpfError {
        let mut error = McOpfError::new(
            McOpfErrorKind::PhysicalValidationFailure,
            McOpfStage::Validation,
            message,
        );
        error.residuals = Some(Box::new(self.residuals.clone()));
        error.violations = self.violations.values().cloned().collect();
        error
    }
    fn mismatch(&mut self, quantity: &'static str, value: f64) -> Result<(), McOpfError> {
        let value = value.abs();
        if !value.is_finite() || value > self.tolerance {
            let witness = McOpfViolation {
                quantity: quantity.into(),
                component: self.component.clone(),
                axis: self.axis.clone(),
                residual: value.is_finite().then_some(value),
            };
            if !value.is_finite()
                || self
                    .violations
                    .get(quantity)
                    .is_none_or(|v| value > v.residual.unwrap_or(f64::INFINITY))
            {
                self.violations.insert(quantity, witness);
            }
        }
        if !value.is_finite() {
            return Err(self.failure(format!(
                "nonfinite independently recomputed {quantity} at {}",
                self.component
            )));
        }
        let target = match quantity {
            "kcl_pu" => &mut self.residuals.kcl_pu,
            "kvl_pu" => &mut self.residuals.kvl_pu,
            "power_link_pu" => &mut self.residuals.power_link_pu,
            "prescribed_power_pu" => &mut self.residuals.prescribed_power_pu,
            "relative_limit_violation" => &mut self.residuals.relative_limit_violation,
            "objective_relative_error" => &mut self.residuals.objective_relative_error,
            _ => return Err(self.failure("unknown physical residual family")),
        };
        *target = target.max(value);
        Ok(())
    }
    fn cap(
        &mut self,
        quantity: &'static str,
        value: f64,
        limit: Option<f64>,
    ) -> Result<(), McOpfError> {
        if !value.is_finite() {
            return self.mismatch(quantity, value);
        }
        if let Some(limit) = limit {
            self.mismatch(
                quantity,
                if limit == 0.0 {
                    value.max(0.0)
                } else {
                    (value / limit - 1.0).max(0.0)
                },
            )?;
        }
        Ok(())
    }
    fn interval(
        &mut self,
        quantity: &'static str,
        value: f64,
        lo: Option<f64>,
        hi: Option<f64>,
    ) -> Result<(), McOpfError> {
        if !value.is_finite() {
            return self.mismatch(quantity, value);
        }
        if let Some(lo) = lo {
            self.mismatch(quantity, (lo - value).max(0.0) / lo.abs().max(1.0))?;
        }
        if let Some(hi) = hi {
            self.mismatch(quantity, (value - hi).max(0.0) / hi.abs().max(1.0))?;
        }
        Ok(())
    }
    fn finish(self) -> Result<McOpfResiduals, McOpfError> {
        if !self.violations.is_empty() {
            return Err(self.failure(format!(
                "independent multiconductor OPF validation failed: {:?}",
                self.residuals
            )));
        }
        Ok(self.residuals)
    }
}
pub(super) fn check(
    model: &Model,
    x: &[f64],
    solver_objective: f64,
    tolerance: f64,
) -> Result<Checked, McOpfError> {
    let mut checks = Checks::new(tolerance);
    if x.len() != model.problem.n
        || x.iter().any(|x| !x.is_finite())
        || !solver_objective.is_finite()
    {
        checks.at(
            "solver point",
            x.iter()
                .position(|v| !v.is_finite())
                .and_then(|i| model.problem.var_names.get(i).cloned()),
        );
        checks.violations.insert(
            "solver_point",
            McOpfViolation {
                quantity: "solver_point".into(),
                component: checks.component.clone(),
                axis: checks.axis.clone(),
                residual: None,
            },
        );
        return Err(checks.failure("invalid or nonfinite solver point/objective"));
    }
    let prep = &model.prep;
    let sb = prep.bases.power_base_va;
    let ib = sb / prep.bases.voltage_base_v;
    let voltage: Vec<_> = model.voltage.iter().map(|v| value(v, x)).collect();
    let mut kcl = vec![C::new(0.0, 0.0); voltage.len()];
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
            checks.at(br.identity.clone(), Some(format!("conductor:{k}")));
            if !br.open {
                let drop: C = currents
                    .iter()
                    .enumerate()
                    .map(|(j, i)| C::new(br.r[k][j], br.x[k][j]) * i)
                    .sum();
                checks.mismatch(
                    "kvl_pu",
                    (voltage[br.from[k]] - voltage[br.to[k]] - drop).norm(),
                )?;
            }
            for (end, nodes, g, b, sign) in [
                (0, &br.from, &br.g_from, &br.b_from, 1.0),
                (1, &br.to, &br.g_to, &br.b_to, -1.0),
            ] {
                checks.at(
                    br.identity.clone(),
                    Some(format!("end:{end}:conductor:{k}")),
                );
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
                checks.cap(
                    "relative_limit_violation",
                    current.norm(),
                    br.current_max[k],
                )?;
                checks.cap("relative_limit_violation", p.norm(), br.apparent_max[k])?;
                if let Some(indices) = power_columns[k][end] {
                    for (index, physical) in indices.into_iter().zip([p.re, p.im]) {
                        checks.mismatch("power_link_pu", x[index] - physical)?;
                    }
                }
                ends[end].push(current);
                powers[end].push(p);
            }
        }
        branches.push(McOpfBranchResult {
            identity: br.identity.clone(),
            from_terminals: br
                .from
                .iter()
                .map(|&n| McOpfTerminalId::at(prep, n))
                .collect(),
            to_terminals: br
                .to
                .iter()
                .map(|&n| McOpfTerminalId::at(prep, n))
                .collect(),
            open: br.open,
            current_from_a: ends[0].iter().map(|v| array(*v, ib)).collect(),
            current_to_a: ends[1].iter().map(|v| array(*v, ib)).collect(),
            power_from_va: powers[0].iter().map(|v| array(*v, sb)).collect(),
            power_to_va: powers[1].iter().map(|v| array(*v, sb)).collect(),
        });
    }
    let mut transformers = Vec::new();
    for (index, ((tx, cols), power_columns)) in prep
        .transformers
        .iter()
        .zip(&model.transformer_current)
        .zip(&model.transformer_power)
        .enumerate()
    {
        checks.at(tx.identity.clone(), Some("tap".into()));
        let currents: Vec<_> = cols.iter().map(|i| value(i, x)).collect();
        let evaluate = |row: &powerio_matrix::McOpfComplexRow| -> C {
            row.voltage
                .iter()
                .map(|&(k, [r, i])| C::new(r, i) * voltage[k])
                .sum::<C>()
                + row
                    .current
                    .iter()
                    .map(|&(k, [r, i])| C::new(r, i) * currents[k])
                    .sum::<C>()
        };
        let alpha = model.transformer_tap[index].map_or(1.0, |id| x[id]);
        if let Some([lo, hi]) = tx.tap_bounds {
            checks.interval("relative_limit_violation", alpha, Some(lo), Some(hi))?;
        }
        for (j, row) in tx.equations.iter().enumerate() {
            checks.at(tx.identity.clone(), Some(format!("equation:{j}")));
            let dynamic = tx.tap_equations.get(j).map_or(C::new(0.0, 0.0), &evaluate);
            checks.mismatch("kvl_pu", (evaluate(row) + alpha * dynamic).norm())?;
        }
        let mut powers = Vec::new();
        for (coil, &current) in tx.coils.iter().zip(&currents) {
            let u = voltage[coil.positive] - coil.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]);
            powers.push(array(u * current.conj(), sb));
            kcl[coil.positive] -= current;
            if let Some(n) = coil.negative {
                kcl[n] += current;
            }
        }
        let mut ports = Vec::new();
        for (port, ids) in tx.ports.iter().zip(power_columns) {
            checks.at(tx.identity.clone(), Some(port.name.clone()));
            let i = evaluate(&port.current);
            let u = voltage[port.positive] - port.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]);
            let p = u * i.conj();
            ports.push(McOpfPortResult {
                name: port.name.clone(),
                positive: McOpfTerminalId::at(prep, port.positive),
                negative: port.negative.map(|n| McOpfTerminalId::at(prep, n)),
                current_a: array(i, ib),
                power_va: array(p, sb),
                current_limit_a: port.current_max.map(|i| i * ib),
                apparent_limit_va: port.apparent_max.map(|s| s * sb),
            });
            checks.cap("relative_limit_violation", i.norm(), port.current_max)?;
            checks.cap("relative_limit_violation", p.norm(), port.apparent_max)?;
            if let Some(ids) = ids {
                checks.mismatch("power_link_pu", (p - C::new(x[ids[0]], x[ids[1]])).norm())?;
            }
        }
        transformers.push(super::McOpfTransformerResult {
            identity: tx.identity.clone(),
            coils: tx
                .coils
                .iter()
                .enumerate()
                .map(|(k, c)| McOpfCoilAxis::new(prep, k, c.positive, c.negative))
                .collect(),
            ports,
            tap: tx.tap_bounds.map(|_| {
                if tx.tap_inverse {
                    tx.tap_nominal / alpha
                } else {
                    tx.tap_nominal * alpha
                }
            }),
            coil_current_a: currents.iter().map(|i| array(*i, ib)).collect(),
            coil_power_va: powers,
        });
    }
    let mut shunts = Vec::new();
    for sh in &prep.shunts {
        let mut currents = Vec::new();
        let mut powers = Vec::new();
        for (k, &node) in sh.terminals.iter().enumerate() {
            let current = sh
                .terminals
                .iter()
                .enumerate()
                .map(|(j, &n)| C::new(sh.g[k][j], sh.b[k][j]) * voltage[n])
                .sum::<C>();
            kcl[node] -= current;
            currents.push(array(current, ib));
            powers.push(array(voltage[node] * current.conj(), sb));
        }
        shunts.push(McOpfShuntResult {
            identity: sh.identity.clone(),
            terminals: sh
                .terminals
                .iter()
                .map(|&n| McOpfTerminalId::at(prep, n))
                .collect(),
            current_a: currents,
            power_va: powers,
        });
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
            checks.at(dev.identity.clone(), Some(format!("coil:{k}")));
            let current = value(&cols.current[k], x);
            let u = voltage[coil.positive] - coil.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]);
            let p = u * current.conj();
            checks.mismatch(
                "power_link_pu",
                (p - C::new(x[cols.power[k][0]], x[cols.power[k][1]])).norm(),
            )?;
            if let Some(law) = &coil.load_law {
                let ratio = u.norm() / law.nominal;
                let impedance = law.terms.iter().flatten().all(|t| t[1] == 2.0);
                if impedance {
                    let admittance = C::new(
                        law.terms[0].iter().map(|t| t[0]).sum(),
                        -law.terms[1].iter().map(|t| t[0]).sum::<f64>(),
                    ) / law.nominal.powi(2);
                    checks.mismatch("prescribed_power_pu", (current - admittance * u).norm())?;
                } else {
                    if ratio <= 0.0 && law.terms.iter().flatten().any(|t| t[1] != 0.0) {
                        return Err(checks.failure(format!(
                            "{} coil {k}: load voltage outside positive domain",
                            dev.identity
                        )));
                    }
                    let target = law
                        .terms
                        .each_ref()
                        .map(|terms| terms.iter().map(|t| t[0] * ratio.powf(t[1])).sum::<f64>());
                    checks.mismatch(
                        "prescribed_power_pu",
                        (p - C::new(target[0], target[1])).norm(),
                    )?;
                }
            } else if let Some(target) = coil.prescribed {
                checks.mismatch(
                    "prescribed_power_pu",
                    (p - C::new(target[0], target[1])).norm(),
                )?;
            }
            checks.interval("relative_limit_violation", p.re, coil.p_min, coil.p_max)?;
            checks.interval("relative_limit_violation", p.im, coil.q_min, coil.q_max)?;
            checks.cap("relative_limit_violation", current.norm(), coil.current_max)?;
            checks.cap("relative_limit_violation", p.norm(), coil.apparent_max)?;
            if let Some([a, b]) = coil.power_factor_coefficients {
                checks.mismatch("prescribed_power_pu", (a * p.re + b * p.im).abs())?;
            }
            for (curve, actual, upper) in [
                (coil.volt_var.as_ref(), p.im, false),
                (coil.volt_watt.as_ref(), p.re, true),
            ] {
                if let Some(curve) = curve {
                    let mean = curve
                        .monitors
                        .iter()
                        .map(|&(p, q)| {
                            (voltage[p] - q.map_or(C::new(0.0, 0.0), |n| voltage[n])).norm()
                        })
                        .sum::<f64>()
                        / (curve.monitors.len() as f64);
                    let relu = |v: f64| {
                        let z = v / curve.epsilon;
                        curve.epsilon * (z.max(0.0) + (-z.abs()).exp().ln_1p())
                    };
                    let mut target = curve.values[0];
                    for j in 0..curve.knots.len() - 1 {
                        target += (curve.values[j + 1] - curve.values[j])
                            / (curve.knots[j + 1] - curve.knots[j])
                            * (relu(mean - curve.knots[j]) - relu(mean - curve.knots[j + 1]));
                    }
                    checks.mismatch(
                        "prescribed_power_pu",
                        if upper {
                            (actual - target).max(0.0)
                        } else {
                            (actual - target).abs()
                        },
                    )?;
                }
            }
            objective += coil.cost * p.re;
            kcl[coil.positive] += sign * current;
            let pos = dev
                .terminals
                .iter()
                .position(|i| *i == coil.positive)
                .ok_or_else(|| {
                    checks.failure(format!(
                        "{}: missing device positive terminal",
                        dev.identity
                    ))
                })?;
            terminal_current[pos] += current;
            if let Some(n) = coil.negative {
                kcl[n] -= sign * current;
                let neg = dev.terminals.iter().position(|i| *i == n).ok_or_else(|| {
                    checks.failure(format!(
                        "{}: missing device negative terminal",
                        dev.identity
                    ))
                })?;
                terminal_current[neg] -= current;
            }
            total += current;
            coil_power.push(array(p, sb));
            coil_current.push(array(current, ib));
        }
        checks.at(dev.identity.clone(), Some("neutral return".into()));
        checks.cap(
            "relative_limit_violation",
            total.norm(),
            dev.neutral_current_max,
        )?;
        if let Some([lo, hi]) = dev.net_active_bounds {
            checks.at(dev.identity.clone(), Some("net active power".into()));
            checks.interval(
                "relative_limit_violation",
                coil_power.iter().map(|p| p[0] / sb).sum(),
                Some(lo),
                Some(hi),
            )?;
        }
        devices.push(McOpfDeviceResult {
            identity: dev.identity.clone(),
            terminals: dev
                .terminals
                .iter()
                .map(|&n| McOpfTerminalId::at(prep, n))
                .collect(),
            coils: dev
                .coils
                .iter()
                .enumerate()
                .map(|(k, c)| McOpfCoilAxis::new(prep, k, c.positive, c.negative))
                .collect(),
            terminal_current_a: terminal_current.iter().map(|&i| array(i, ib)).collect(),
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
        checks.at(limit.identity.clone(), None);
        let v = if limit.combination.is_empty() {
            (voltage[limit.positive] - limit.negative.map_or(C::new(0.0, 0.0), |n| voltage[n]))
                .norm()
        } else {
            limit
                .combination
                .iter()
                .map(|&(n, [r, i])| C::new(r, i) * voltage[n])
                .sum::<C>()
                .norm()
        };
        checks.cap("relative_limit_violation", v, limit.upper)?;
        if let Some(floor) = limit.lower.filter(|x| *x > 0.0) {
            checks.mismatch("relative_limit_violation", (1.0 - v / floor).max(0.0))?;
        }
    }
    for angle in &prep.angle_limits {
        checks.at(angle.identity.clone(), None);
        let z =
            voltage[angle.first] * voltage[angle.second].conj() * C::from_polar(1.0, -angle.offset);
        if z.norm() > 1e-12 {
            checks.interval(
                "relative_limit_violation",
                z.arg(),
                Some(angle.lower),
                Some(angle.upper),
            )?;
        }
    }
    for (i, t) in prep.terminals.iter().enumerate() {
        if !t.grounded {
            checks.at(t.bus.clone(), Some(t.terminal.clone()));
            checks.mismatch("kcl_pu", kcl[i].norm())?;
        }
    }
    checks.at("objective", None);
    checks.mismatch(
        "objective_relative_error",
        (objective - solver_objective) / objective.abs().max(1.0),
    )?;
    let residuals = checks.finish()?;
    Ok(Checked {
        voltage,
        shunts,
        residuals,
        branches,
        transformers,
        devices,
        objective,
    })
}

/// Recheck the declared PF using physical ledgers and original input profiles,
/// independently of prepared coefficients and the expression graph.
pub(super) fn check_power_factors(
    network: &powerio::MulticonductorNetwork,
    devices: &[McOpfDeviceResult],
    power_base: f64,
    tolerance: f64,
) -> Result<Vec<super::McOpfDiagnostic>, McOpfError> {
    let mut checks = Checks::new(tolerance);
    let mut diagnostics = Vec::new();
    for inv in network.ibrs() {
        let Some(pf) = inv
            .control_profile
            .as_ref()
            .and_then(|name| network.control_profiles().iter().find(|c| &c.name == name))
            .and_then(|c| c.power_factor.as_ref())
            .map(|c| c.pf)
        else {
            continue;
        };
        let device = devices
            .iter()
            .find(|d| d.identity == inv.name && d.kind == McOpfDeviceKind::Ibr)
            .ok_or_else(|| checks.failure("missing inverter PF ledger"))?;
        // Product of square roots is independent of preparation's product form.
        let a = (1. - pf.abs()).sqrt() * (1. + pf.abs()).sqrt();
        for (k, &[p, q]) in device.coil_power_va.iter().enumerate() {
            checks.at(inv.name.clone(), Some(format!("coil:{k}")));
            checks.mismatch(
                "prescribed_power_pu",
                a * (p / power_base) + pf * (q / power_base),
            )?;
            let apparent = p.hypot(q);
            let achieved = (apparent > 0.).then(|| p.abs() / apparent);
            let relative_error = achieved.map(|v| (v - pf.abs()).abs() / pf.abs());
            let wrong_sign =
                a > 0. && p != 0. && q != 0. && p.signum() * q.signum() != -pf.signum();
            if pf.abs() <= tolerance
                || apparent == 0.
                || wrong_sign
                || relative_error.is_some_and(|v| v > tolerance)
            {
                let achieved_text =
                    achieved.map_or_else(|| "undefined".into(), |v| format!("{v:e}"));
                diagnostics.push(super::McOpfDiagnostic {
                    code: "MC_OPF.POWER_FACTOR_ACCURACY_UNQUALIFIED".into(),
                    severity: powerio::DiagnosticSeverity::Warning,
                    message: format!("coil {k}: requested signed PF {pf:e}, achieved |P|/|S| {achieved_text}; the absolute power-equation tolerance does not qualify relative PF accuracy or its P/Q sign relation (undefined at zero dispatch)"),
                    target: Some(inv.name.clone()),
                });
            }
        }
    }
    checks.finish()?;
    Ok(diagnostics)
}
