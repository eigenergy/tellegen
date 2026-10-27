//! Numerical seed projection. This module may change x0, never equations,
//! bounds, network data, or the physical preparation fingerprint.
use super::{
    model::{Model, Pair, StartLift},
    *,
};
use num_complex::Complex64 as C;
use powerio::{McAcPfSolution, MulticonductorNetwork};
use powerio_prob::{
    MulticonductorOperatingPointBuilder, MulticonductorOperatingPointFlag as Flag,
    MulticonductorOperatingPointQuantity as Q, OperatingPoint,
};
use std::collections::{BTreeMap, BTreeSet};

const QUANTITIES: [Q; 7] = [
    Q::TerminalVoltageMagnitude,
    Q::TerminalVoltageAngle,
    Q::LoadActivePower,
    Q::LoadReactivePower,
    Q::GeneratorActivePower,
    Q::GeneratorReactivePower,
    Q::TransformerTap,
];

/// Extract voltage guesses from a converged PowerIO MC PF solution. The OPF
/// instance still owns the physical equations; compatibility is checked by
/// identity when the point is attached with `with_initial_point` and solved.
pub fn mc_opf_initial_point_from_pf(
    solution: &McAcPfSolution,
) -> Result<OperatingPoint<MulticonductorNetwork>, McOpfError> {
    if *solution.termination() != Termination::Converged {
        return Err(invalid(
            "pf_solution",
            "requires a converged MC PF solution",
        ));
    }
    let network = solution.instance().network();
    let mut vm = Vec::new();
    let mut va = Vec::new();
    for bus in network.buses() {
        for terminal in &bus.terminals {
            let m = solution
                .terminal_voltage_magnitude(&bus.id, terminal)
                .ok_or_else(|| invalid("pf_solution", "missing terminal magnitude"))?;
            let a = solution
                .terminal_voltage_angle(&bus.id, terminal)
                .ok_or_else(|| invalid("pf_solution", "missing terminal angle"))?;
            if !m.is_finite() || m < 0.0 || !a.is_finite() {
                return Err(invalid(
                    &format!("{}/{}", bus.id, terminal),
                    "invalid PF voltage",
                ));
            }
            vm.push(m);
            va.push(a);
        }
    }
    MulticonductorOperatingPointBuilder::for_point(network.clone())
        .terminal_voltage_magnitudes(vm)
        .terminal_voltage_angles(va)
        .build_point()
        .map_err(|e| McOpfError::from_powerio(McOpfStage::Input, e))
}

/// Released PowerIO preparation refuses initial points because it does not
/// project optimizer variables. Give it the unchanged physical problem and
/// retain the declared point on the original instance returned in the solution.
pub(super) fn prepare(
    instance: &McAcOpfInstance,
    bases: &McAcOpfAssemblyOptions,
) -> Result<powerio_matrix::McAcOpfPreparation, McOpfError> {
    if let Some(point) = instance.initial_point() {
        validate_identities(point, instance.network())?;
    }
    let physical;
    let input = if instance.initial_point().is_some() {
        physical = McAcOpfInstance::from_network(instance.network().clone())
            .map_err(|e| McOpfError::from_powerio(McOpfStage::Preparation, e))?
            .with_objective(instance.objective().clone())
            .with_constraints(instance.constraints().clone());
        &physical
    } else {
        instance
    };
    build_mc_ac_opf_preparation(input, bases)
        .map_err(|e| McOpfError::from_powerio(McOpfStage::Preparation, e))
}

fn invalid(target: &str, message: &str) -> McOpfError {
    let mut error = McOpfError::new(
        McOpfErrorKind::InvalidInput,
        McOpfStage::Input,
        format!("initial point {target}: {message}"),
    );
    error.diagnostics.push(McOpfDiagnostic {
        code: "MC_OPF.INITIAL_POINT_INVALID".into(),
        severity: powerio::DiagnosticSeverity::Error,
        message: error.message.clone(),
        target: Some(target.into()),
    });
    error
}
fn unsupported(target: &str) -> McOpfError {
    let mut error = invalid(
        target,
        "is equipment state, not a continuous IVR starting variable",
    );
    error.kind = McOpfErrorKind::UnsupportedInput;
    error.diagnostics[0].code = "MC_OPF.INITIAL_POINT_UNSUPPORTED".into();
    error
}
fn remark(target: String, message: &str) -> McOpfDiagnostic {
    McOpfDiagnostic {
        code: "MC_OPF.INITIAL_POINT_PROJECTED".into(),
        severity: powerio::DiagnosticSeverity::Remark,
        message: message.into(),
        target: Some(target),
    }
}
fn value(pair: &Pair, x: &[f64]) -> C {
    C::new(pair[0].value(x), pair[1].value(x))
}
fn set(pair: &Pair, v: C, x: &mut [f64]) {
    for (a, v) in pair.iter().zip([v.re, v.im]) {
        // Compilation exposes either one independent column or a fixed value.
        if let Some((&i, &coefficient)) = a.terms.first_key_value() {
            debug_assert_eq!(a.terms.len(), 1);
            x[i] = (v - a.constant) / coefficient;
        }
    }
}

fn validate_identities(
    point: &OperatingPoint<MulticonductorNetwork>,
    net: &MulticonductorNetwork,
) -> Result<(), McOpfError> {
    for q in QUANTITIES {
        let Some(values) = point.values(q) else {
            continue;
        };
        let expected: BTreeSet<_> = identities(net, q).into_iter().collect();
        let actual: BTreeSet<_> = values.clone().map(|(id, _)| id.to_owned()).collect();
        if actual != expected {
            return Err(invalid(
                q.name(),
                "component/terminal identities differ from the declared instance",
            ));
        }
        for (id, v) in values {
            if !v.is_finite()
                || (q == Q::TerminalVoltageMagnitude && v < 0.0)
                || (q == Q::TransformerTap && v <= 0.0)
            {
                return Err(invalid(
                    &format!("{}/{id}", q.name()),
                    "value is nonfinite or outside its physical coordinate domain",
                ));
            }
        }
    }
    if point.flags(Flag::SwitchClosed).is_some() {
        return Err(unsupported("switch_closed"));
    }
    if point.values(Q::CapacitorSteps).is_some() {
        return Err(unsupported("capacitor_steps"));
    }
    Ok(())
}

fn identities(net: &MulticonductorNetwork, q: Q) -> Vec<String> {
    match q {
        Q::TerminalVoltageMagnitude | Q::TerminalVoltageAngle => net
            .buses()
            .iter()
            .flat_map(|b| b.terminals.iter().map(move |t| format!("{}/{t}", b.id)))
            .collect(),
        Q::LoadActivePower | Q::LoadReactivePower => net
            .loads()
            .iter()
            .flat_map(|d| {
                d.terminal_map
                    .iter()
                    .map(move |t| format!("{}/{t}", d.name))
            })
            .collect(),
        Q::GeneratorActivePower | Q::GeneratorReactivePower => net
            .generators()
            .iter()
            .flat_map(|d| {
                d.terminal_map
                    .iter()
                    .map(move |t| format!("{}/{t}", d.name))
            })
            .collect(),
        Q::TransformerTap => net.transformers().iter().map(|t| t.name.clone()).collect(),
        _ => unreachable!(),
    }
}

/// PowerIO's stored assignment decoder requires the instance's column order.
/// Preserve every supplied value/family, but canonicalize their storage order
/// so a typed solve with a reordered seed can round-trip its returned instance.
pub(super) fn canonical_instance(
    instance: Arc<McAcOpfInstance>,
) -> Result<Arc<McAcOpfInstance>, McOpfError> {
    let Some(point) = instance.initial_point() else {
        return Ok(instance);
    };
    let mut builder = MulticonductorOperatingPointBuilder::for_point(instance.network().clone());
    for q in QUANTITIES {
        let Some(values) = point.values(q) else {
            continue;
        };
        let values: BTreeMap<_, _> = values.collect();
        let ordered = identities(instance.network(), q)
            .iter()
            .map(|id| {
                values
                    .get(id.as_str())
                    .copied()
                    .ok_or_else(|| invalid(id, "missing identity during export"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        builder = match q {
            Q::TerminalVoltageMagnitude => builder.terminal_voltage_magnitudes(ordered),
            Q::TerminalVoltageAngle => builder.terminal_voltage_angles(ordered),
            Q::LoadActivePower => builder.load_active_powers(ordered),
            Q::LoadReactivePower => builder.load_reactive_powers(ordered),
            Q::GeneratorActivePower => builder.generator_active_powers(ordered),
            Q::GeneratorReactivePower => builder.generator_reactive_powers(ordered),
            Q::TransformerTap => builder.transformer_taps(ordered),
            _ => unreachable!(),
        };
    }
    let point = builder
        .build_point()
        .map_err(|e| McOpfError::from_powerio(McOpfStage::Export, e))?;
    Ok(Arc::new((*instance).clone().with_initial_point(point)))
}

/// Project only the independent starting vector. Auxiliary definitions are then
/// evaluated at that vector; no seed is ever substituted into a physical row.
pub(super) fn apply(
    model: &mut Model,
    instance: &McAcOpfInstance,
) -> Result<Vec<McOpfDiagnostic>, McOpfError> {
    let Some(point) = instance.initial_point() else {
        return Ok(Vec::new());
    };
    validate_identities(point, instance.network())?;
    let prep = &model.prep;
    let vb = prep.bases.voltage_base_v;
    let sb = prep.bases.power_base_va;
    let mut x = model.problem.x0.clone();
    let mut diagnostics = Vec::new();
    for (terminal, pair) in prep.terminals.iter().zip(&model.voltage) {
        let old = value(pair, &x);
        let magnitude = point.terminal_voltage_magnitude(&terminal.bus, &terminal.terminal);
        let angle = point.terminal_voltage_angle(&terminal.bus, &terminal.terminal);
        if magnitude.is_none() && angle.is_none() {
            continue;
        }
        let v = C::from_polar(
            magnitude.map_or(old.norm(), |m| m / vb),
            angle.unwrap_or(old.arg()),
        );
        if terminal.fixed.is_some() && (v - old).norm() > 1e-10 {
            diagnostics.push(remark(
                format!("{}/{}", terminal.bus, terminal.terminal),
                "fixed source/ground voltage takes precedence over the numerical guess",
            ));
        }
        set(pair, v, &mut x);
    }
    for ((tx, tap), currents) in prep
        .transformers
        .iter()
        .zip(&model.transformer_tap)
        .zip(&model.transformer_current)
    {
        if let Some(physical) = point.transformer_tap(&tx.identity) {
            if let Some(id) = tap {
                x[*id] = if tx.tap_inverse {
                    tx.tap_nominal / physical
                } else {
                    physical / tx.tap_nominal
                };
            } else {
                diagnostics.push(remark(
                    tx.identity.clone(),
                    "fixed transformer taps stay at their declared equipment values",
                ));
            }
        }
        let alpha = tap.map_or(1.0, |i| x[i]);
        let mut rows = Vec::new();
        let mut rhs = Vec::new();
        for (k, equation) in tx.equations.iter().enumerate() {
            let mut coefficients = vec![C::default(); currents.len()];
            let mut constant = C::default();
            for (row, factor) in
                std::iter::once((equation, 1.0)).chain(tx.tap_equations.get(k).map(|r| (r, alpha)))
            {
                for &(i, [r, im]) in &row.voltage {
                    constant += factor * C::new(r, im) * value(&model.voltage[i], &x);
                }
                for &(i, [r, im]) in &row.current {
                    coefficients[i] += factor * C::new(r, im);
                }
            }
            complex_row(&coefficients, -constant, &mut rows, &mut rhs);
        }
        if let Some(seed) = project(&rows, &rhs, &vec![0.0; currents.len() * 2]) {
            for (pair, v) in currents.iter().zip(seed.chunks_exact(2)) {
                set(pair, C::new(v[0], v[1]), &mut x);
            }
        }
        // Inconsistent voltage guesses or ideal/singular equipment need not
        // admit inferred currents. The unspecified currents retain zero seeds.
    }
    for (br, currents) in prep.branches.iter().zip(&model.branch_current) {
        if br.open {
            continue;
        }
        let mut rows = Vec::new();
        let mut rhs = Vec::new();
        for k in 0..currents.len() {
            let coefficients: Vec<_> = br.r[k]
                .iter()
                .zip(&br.x[k])
                .map(|(&r, &i)| C::new(r, i))
                .collect();
            complex_row(
                &coefficients,
                value(&model.voltage[br.from[k]], &x) - value(&model.voltage[br.to[k]], &x),
                &mut rows,
                &mut rhs,
            );
        }
        if let Some(seed) = project(&rows, &rhs, &vec![0.0; currents.len() * 2]) {
            for (pair, v) in currents.iter().zip(seed.chunks_exact(2)) {
                set(pair, C::new(v[0], v[1]), &mut x);
            }
        }
    }
    for (dev, cols) in prep.devices.iter().zip(&model.devices) {
        // Defaults follow the physical load law at the seeded voltage. Explicit
        // terminal powers below override only the numerical current guesses.
        for (coil, pair) in dev.coils.iter().zip(&cols.current) {
            let u = value(&model.voltage[coil.positive], &x)
                - coil
                    .negative
                    .map_or(C::default(), |n| value(&model.voltage[n], &x));
            let p = if let Some(law) = &coil.load_law {
                let ratio = u.norm() / law.nominal;
                let powers = law
                    .terms
                    .each_ref()
                    .map(|terms| terms.iter().map(|t| t[0] * ratio.powf(t[1])).sum::<f64>());
                C::new(powers[0], powers[1])
            } else {
                let p = coil.prescribed.unwrap_or([0.0, 0.0]);
                C::new(p[0], p[1])
            };
            set(
                pair,
                if u.norm_sqr() > 0.0 {
                    (p / u).conj()
                } else {
                    C::default()
                },
                &mut x,
            );
        }
        let mut rows = Vec::new();
        let mut rhs = Vec::new();
        for &n in &dev.terminals {
            let terminal = &prep.terminals[n].terminal;
            let powers = match dev.kind {
                powerio_matrix::McOpfDeviceKind::Load => [
                    point.load_active_power(&dev.identity, terminal),
                    point.load_reactive_power(&dev.identity, terminal),
                ],
                powerio_matrix::McOpfDeviceKind::Generator => [
                    point.generator_active_power(&dev.identity, terminal),
                    point.generator_reactive_power(&dev.identity, terminal),
                ],
                _ => [None, None],
            };
            let v = value(&model.voltage[n], &x);
            for (reactive, power) in powers.into_iter().enumerate() {
                if let Some(power) = power {
                    let mut row = vec![0.0; dev.coils.len() * 2];
                    for (k, c) in dev.coils.iter().enumerate() {
                        let incidence =
                            f64::from(c.positive == n) - f64::from(c.negative == Some(n));
                        row[2 * k] = incidence * if reactive == 0 { v.re } else { v.im };
                        row[2 * k + 1] = incidence * if reactive == 0 { v.im } else { -v.re };
                    }
                    rows.push(row);
                    rhs.push(power / sb);
                }
            }
        }
        if !rows.is_empty() {
            let default: Vec<_> = cols
                .current
                .iter()
                .flat_map(|i| {
                    let v = value(i, &x);
                    [v.re, v.im]
                })
                .collect();
            let seed = project(&rows, &rhs, &default).ok_or_else(|| invalid(&dev.identity, "terminal power guesses are inconsistent with coil incidence at the seeded voltages"))?;
            for (pair, v) in cols.current.iter().zip(seed.chunks_exact(2)) {
                set(pair, C::new(v[0], v[1]), &mut x);
            }
        }
    }
    refresh(model, &mut x)?;
    model.problem.x0 = x;
    Ok(diagnostics)
}

/// Rebuild dependent coordinates after a session carries independent primals
/// into a newly compiled problem. Zero reductions always take precedence.
pub(super) fn refresh(model: &Model, x: &mut [f64]) -> Result<(), McOpfError> {
    for &id in &model.zero_columns {
        x[id] = 0.0;
    }
    for lift in &model.start_lifts {
        match lift {
            StartLift::Power(ids, u, i) => {
                let p = value(u, x) * value(i, x).conj();
                x[ids[0]] = p.re;
                x[ids[1]] = p.im;
            }
            StartLift::Magnitude(id, u) => x[*id] = value(u, x).norm(),
            StartLift::LogVoltage(id, u, nominal) => {
                let ratio = value(u, x).norm() / nominal;
                if ratio <= 0.0 {
                    return Err(invalid(
                        &model.problem.var_names[*id],
                        "a positive voltage is required for the logarithmic load-law seed",
                    ));
                }
                x[*id] = ratio.ln();
            }
            StartLift::Voltage(to, from) => {
                let v = value(from, x);
                set(to, v, x);
            }
        }
    }
    for &id in &model.zero_columns {
        x[id] = 0.0;
    }
    if let Some(i) = x.iter().position(|v| !v.is_finite()) {
        return Err(invalid(
            &model.problem.var_names[i],
            "projected starting value is not finite",
        ));
    }
    Ok(())
}

fn complex_row(coefficients: &[C], rhs: C, rows: &mut Vec<Vec<f64>>, values: &mut Vec<f64>) {
    rows.push(coefficients.iter().flat_map(|c| [c.re, -c.im]).collect());
    values.push(rhs.re);
    rows.push(coefficients.iter().flat_map(|c| [c.im, c.re]).collect());
    values.push(rhs.im);
}

/// Small local rank-aware projection onto A*x=b, retaining the default in the
/// nullspace. Two orthogonalization passes avoid squared-condition-number normal
/// equations. This only initializes guesses; it is not an electrical solver.
fn project(rows: &[Vec<f64>], rhs: &[f64], default: &[f64]) -> Option<Vec<f64>> {
    let mut basis: Vec<(Vec<f64>, f64)> = Vec::new();
    for (row, &target) in rows.iter().zip(rhs) {
        let scale = row.iter().map(|v| v.abs()).fold(0.0, f64::max);
        if !scale.is_finite() || !target.is_finite() {
            return None;
        }
        if scale == 0.0 {
            if target.abs() > 1e-10 {
                return None;
            } else {
                continue;
            }
        }
        let mut q: Vec<_> = row.iter().map(|v| v / scale).collect();
        let mut b = target / scale;
        for _ in 0..2 {
            for (prev, value) in &basis {
                let dot: f64 = q.iter().zip(prev).map(|(a, b)| a * b).sum();
                for (a, p) in q.iter_mut().zip(prev) {
                    *a -= dot * p;
                }
                b -= dot * value;
            }
        }
        let norm = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm > 1e-10 {
            basis.push((q.iter().map(|v| v / norm).collect(), b / norm));
        }
    }
    let mut x = default.to_vec();
    for (q, b) in basis {
        let correction = b - q.iter().zip(default).map(|(a, b)| a * b).sum::<f64>();
        for (x, q) in x.iter_mut().zip(q) {
            *x += correction * q;
        }
    }
    if x.iter().any(|v| !v.is_finite()) {
        return None;
    }
    for (row, &b) in rows.iter().zip(rhs) {
        let actual: f64 = row.iter().zip(&x).map(|(a, b)| a * b).sum();
        if !actual.is_finite() || (actual - b).abs() > 1e-8 * (1.0 + b.abs()) {
            return None;
        }
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::project;

    #[test]
    fn rank_deficient_projection_preserves_nullspace_and_detects_conflicting_rows() {
        // x+y=4 fixes the sum; the default's difference and third coordinate
        // should survive. Reordering and scaling redundant rows must not matter.
        for (rows, rhs) in [
            (vec![vec![1., 1., 0.], vec![2., 2., 0.]], vec![4., 8.]),
            (vec![vec![2e-6, 2e-6, 0.], vec![1., 1., 0.]], vec![8e-6, 4.]),
        ] {
            let x = project(&rows, &rhs, &[3., 1., 7.]).unwrap();
            for (actual, expected) in x.iter().zip([3., 1., 7.]) {
                assert!((actual - expected).abs() < 1e-10);
            }
            let x = project(&rows, &rhs, &[5., 1., 7.]).unwrap();
            for (actual, expected) in x.iter().zip([4., 0., 7.]) {
                assert!((actual - expected).abs() < 1e-10);
            }
        }
        assert!(project(&[vec![1., 1.], vec![2., 2.]], &[4., 9.], &[0., 0.]).is_none());
        assert!(project(&[vec![0., 0.]], &[1.], &[0., 0.]).is_none());
    }
}
