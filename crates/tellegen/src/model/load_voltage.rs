//! Voltage-dependent loads (ZIP and exponential) in an AC power flow whose
//! equations take constant-power loads: each such load is evaluated at the
//! bus voltage, the flow is solved with it as constant power, and this repeats
//! until the voltages settle. At the fixed point the loads draw exactly their
//! voltage-dependent power. The static AC power flow and the RMS
//! simulation's initialization share it, so both find the same operating
//! point.

use std::collections::BTreeMap;

use powerio::{BalancedNetwork, Load, LoadVoltageModel};

use crate::api::SolveDiagnostic;

/// Fixed-point rounds before giving up.
const MAX_ITERATIONS: usize = 30;
/// Largest voltage magnitude change (pu) between rounds at the fixed point.
const TOLERANCE: f64 = 1e-10;

/// The in-service loads whose power depends on the voltage.
pub(crate) fn voltage_dependent(network: &BalancedNetwork) -> Vec<usize> {
    network
        .loads()
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            l.in_service
                && matches!(
                    l.voltage_model,
                    Some(LoadVoltageModel::Zip { .. } | LoadVoltageModel::Exponential { .. })
                )
        })
        .map(|(i, _)| i)
        .collect()
}

/// A load's `(p, q)` at the bus voltages `v` (bus id → pu; 1 pu where none).
/// A nominal voltage below 2 is in per unit; one of 2 or more is in kV and is
/// taken against the bus's base voltage `base_kv` (bus id → kV), which must
/// then be known.
fn power_at(
    load: &Load,
    v: &BTreeMap<usize, f64>,
    base_kv: &BTreeMap<usize, f64>,
) -> Result<(f64, f64), String> {
    let vb = v.get(&load.bus.0).copied().unwrap_or(1.0);
    let ratio = |v_nom: Option<f64>| -> Result<f64, String> {
        match v_nom {
            None => Ok(vb),
            Some(n) if n > 0.0 && n < 2.0 => Ok(vb / n),
            Some(n) if n >= 2.0 => match base_kv.get(&load.bus.0) {
                Some(&kv) if kv > 0.0 => Ok(vb * kv / n),
                _ => Err(format!(
                    "the load at bus {} states a nominal voltage of {n} kV, but its bus has no \
                     base voltage",
                    load.bus.0
                )),
            },
            Some(n) => Err(format!(
                "the load at bus {} states a nominal voltage of {n}",
                load.bus.0
            )),
        }
    };
    match load.voltage_model.as_ref() {
        Some(LoadVoltageModel::Zip {
            p_constant_power,
            q_constant_power,
            p_constant_current,
            q_constant_current,
            p_constant_impedance,
            q_constant_impedance,
            v_nom,
            ..
        }) => {
            let r = ratio(*v_nom)?;
            Ok((
                p_constant_power + p_constant_current * r + p_constant_impedance * r * r,
                q_constant_power + q_constant_current * r + q_constant_impedance * r * r,
            ))
        }
        Some(LoadVoltageModel::Exponential {
            p,
            q,
            v_nom,
            gamma_p,
            gamma_q,
        }) => {
            let r = ratio(*v_nom)?;
            Ok((p * r.powf(*gamma_p), q * r.powf(*gamma_q)))
        }
        _ => Ok((load.p, load.q)),
    }
}

/// Solve a power flow of `network` with its voltage-dependent loads at their
/// power at the solved voltages. `solve` takes a network whose loads are all
/// constant power and returns its result with the bus voltage magnitudes
/// `(bus id, vm)`. Returns that result at the fixed point and the number of
/// rounds (0 when every load is constant power: `solve` ran once on
/// `network` itself).
///
/// # Errors
/// What `solve` returns, or when the voltages do not settle.
pub(crate) fn settle<T>(
    network: &BalancedNetwork,
    mut solve: impl FnMut(&BalancedNetwork) -> Result<(T, Vec<(usize, f64)>), String>,
) -> Result<(T, usize), String> {
    let dependent = voltage_dependent(network);
    if dependent.is_empty() {
        return solve(network).map(|(t, _)| (t, 0));
    }
    let mut working = network.clone();
    let mut v: BTreeMap<usize, f64> = network.buses().iter().map(|b| (b.id.0, 1.0)).collect();
    let base_kv: BTreeMap<usize, f64> = network
        .buses()
        .iter()
        .map(|b| (b.id.0, b.base_kv))
        .collect();
    for iteration in 1..=MAX_ITERATIONS {
        for &i in &dependent {
            let (p, q) = power_at(&network.loads()[i], &v, &base_kv)?;
            let load = &mut working.loads_mut()[i];
            load.p = p;
            load.q = q;
            load.voltage_model = Some(LoadVoltageModel::ConstantPower);
        }
        let (result, vm) = solve(&working)?;
        let mut change: f64 = 0.0;
        for (number, x) in vm {
            let old = v.insert(number, x).unwrap_or(1.0);
            change = change.max((x - old).abs());
        }
        if change < TOLERANCE {
            return Ok((result, iteration));
        }
    }
    Err(format!(
        "the power flow with voltage-dependent loads did not settle in {MAX_ITERATIONS} iterations"
    ))
}

/// What a solve with voltage-dependent loads reports about them.
pub(crate) fn diagnostic(network: &BalancedNetwork, iterations: usize) -> Option<SolveDiagnostic> {
    let dependent = voltage_dependent(network);
    if dependent.is_empty() {
        return None;
    }
    let mut buses: Vec<usize> = dependent
        .iter()
        .map(|&i| network.loads()[i].bus.0)
        .collect();
    buses.sort_unstable();
    buses.dedup();
    Some(SolveDiagnostic {
        code: "load_voltage_dependent".to_owned(),
        message: format!(
            "{} voltage-dependent (ZIP or exponential) load(s) draw their power at the solved \
             voltages ({iterations} fixed-point rounds); sensitivities and previews hold them at \
             that power",
            dependent.len()
        ),
        buses,
    })
}

