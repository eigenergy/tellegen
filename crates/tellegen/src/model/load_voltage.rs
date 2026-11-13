//! Voltage-dependent loads (ZIP and exponential) in the AC power flow.
//!
//! PowerIO states a load's voltage dependence on [`Load::voltage_model`]:
//!
//! - [`LoadVoltageModel::Zip`] splits the load into constant-power,
//!   constant-current and constant-impedance parts, each in the load's own unit
//!   (MW / MVAr, or per unit on the system base in a normalized network), each
//!   the part's power at the nominal voltage. The parts sum to [`Load::p`] and
//!   [`Load::q`].
//! - [`LoadVoltageModel::Exponential`] states `P = p (V / v_nom)^gamma_p`,
//!   `Q = q (V / v_nom)^gamma_q`, again in the load's own unit.
//!
//! The power flow carries each load exactly. With `x = vm / vn` (the bus
//! voltage magnitude against the load's nominal voltage, both per unit on the
//! bus base), the bus demand is
//!
//! ```text
//! P_L(vm) = pd + Σ_terms c_k x^γ_k            Q_L(vm) = qd + Σ_terms c_k x^γ_k
//! ```
//!
//! where `pd`/`qd` hold every constant-power part (constant-power loads, the
//! ZIP constant-power parts, exponential loads with `γ = 0`) and each term is a
//! ZIP constant-current part (`γ = 1`), a ZIP constant-impedance part (`γ = 2`)
//! or an exponential load (its `γ`). The Newton mismatch evaluates `P_L` and
//! `Q_L` at the current iterate and the Jacobian carries their exact
//! derivative `dc_k x^γ_k / dvm = γ_k c_k x^(γ_k − 1) / vn` (see
//! [`LoadTerm::eval`]), so the solve converges quadratically to the operating
//! point at which every load draws its voltage-dependent power, and the AC
//! sensitivities differentiate the same system.
//!
//! A load's nominal voltage `v_nom` is optional; without one it is the bus
//! base voltage (`vn = 1` per unit), the convention every PowerIO reader
//! writes. When a source states one, PowerIO does not state its unit, so
//! [`nominal_pu`] reads it against the bus base voltage.

use std::collections::BTreeMap;

use powerio::{BalancedNetwork, BusType, LoadVoltageModel};

use crate::api::SolveDiagnostic;

/// Below this ratio `vm / vn` a term with exponent below one is continued by
/// its tangent at the floor (see [`LoadTerm::eval`]): `x^γ` has an unbounded
/// slope at zero for `γ < 1` (and is itself unbounded for `γ < 0`), and a
/// Newton iterate can pass near zero on its way to the solution. A converged
/// point with any voltage-dependent term below the floor is refused rather than
/// reported: for `γ < 1` it was solved on the continuation, and for `γ ≥ 1`
/// it is the collapsed root `vm = 0` that such a load admits at any loading.
pub(crate) const RATIO_FLOOR: f64 = 1e-3;

/// A voltage-dependent part of a bus's demand: `coefficient · (vm / v_nom)^exponent`
/// in per unit on the system base, added to the bus's active (or reactive) demand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LoadTerm {
    /// Dense bus index.
    pub bus: usize,
    /// Reactive (`true`) or active (`false`) demand.
    pub reactive: bool,
    /// The part's power at the nominal voltage (per unit on the system base).
    pub coefficient: f64,
    /// The nominal voltage (per unit on the bus base), positive.
    pub v_nom: f64,
    /// The voltage exponent (1 constant current, 2 constant impedance).
    pub exponent: f64,
}

impl LoadTerm {
    /// The term's power at magnitude `vm` (per unit) and its derivative w.r.t. `vm`:
    /// with `x = vm / v_nom`, `(c x^γ, γ c x^(γ−1) / v_nom)`. Exponents 1 and 2 are
    /// evaluated without `powf`. For `γ < 1` and `x` below [`RATIO_FLOOR`] the term
    /// follows its tangent at the floor, `c (f₀ + f₀' (x − x₀))`, so value and slope
    /// stay finite and consistent as an iterate approaches zero voltage.
    pub(crate) fn eval(&self, vm: f64) -> (f64, f64) {
        let x = vm / self.v_nom;
        let (c, g, vn) = (self.coefficient, self.exponent, self.v_nom);
        if g == 1.0 {
            (c * x, c / vn)
        } else if g == 2.0 {
            (c * x * x, 2.0 * c * x / vn)
        } else if g < 1.0 && x < RATIO_FLOOR {
            let f0 = RATIO_FLOOR.powf(g);
            let d0 = g * RATIO_FLOOR.powf(g - 1.0);
            (c * (f0 + d0 * (x - RATIO_FLOOR)), c * d0 / vn)
        } else {
            (c * x.powf(g), c * g * x.powf(g - 1.0) / vn)
        }
    }

    /// Whether `vm` lies below the term's ratio floor: a collapsed voltage, not an
    /// operating point (see [`RATIO_FLOOR`]).
    pub(crate) fn below_floor(&self, vm: f64) -> bool {
        self.exponent != 0.0 && vm / self.v_nom < RATIO_FLOOR
    }
}

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

/// A load's nominal voltage in per unit on its bus base.
///
/// No stated `v_nom` is the bus base voltage (1 per unit). PowerIO's model does
/// not state the unit of a stated one, and sources use both, so it is read
/// against the bus base voltage `base_kv`: as kV giving `v_nom / base_kv`, or as
/// per unit giving `v_nom`. A reading is plausible in `[0.5, 2]` and usual in
/// `[0.8, 1.25]`. When the two readings agree, or only one is plausible, or only
/// one is usual (`v_nom = 0.69` on a 0.69 kV bus is 1 pu in kV, not 0.69 pu), that
/// reading is taken. When both are usual and differ (a 0.9 kV bus with
/// `v_nom = 1`), both are plausible and neither usual, or neither is plausible, the
/// load is refused rather than solved at a guessed voltage.
///
/// # Errors
/// A non-positive or non-finite `v_nom`, or one no reading makes plausible or
/// two readings make plausible differently.
pub(crate) fn nominal_pu(v_nom: Option<f64>, base_kv: f64, bus: usize) -> Result<f64, String> {
    let Some(stated) = v_nom else {
        return Ok(1.0);
    };
    if !(stated.is_finite() && stated > 0.0) {
        return Err(format!(
            "the voltage-dependent load at bus {bus} states a nominal voltage of {stated}"
        ));
    }
    let plausible = |ratio: f64| (0.5..=2.0).contains(&ratio);
    let usual = |ratio: f64| (0.8..=1.25).contains(&ratio);
    let mut as_kv = (base_kv.is_finite() && base_kv > 0.0)
        .then(|| stated / base_kv)
        .filter(|&ratio| plausible(ratio));
    let mut as_pu = plausible(stated).then_some(stated);
    if let (Some(kv), Some(pu)) = (as_kv, as_pu) {
        match (usual(kv), usual(pu)) {
            (true, false) => as_pu = None,
            (false, true) => as_kv = None,
            _ => {}
        }
    }
    match (as_kv, as_pu) {
        (Some(kv), Some(pu)) if (kv - pu).abs() <= 1e-9 * kv.max(pu) => Ok(kv),
        (Some(kv), None) => Ok(kv),
        (None, Some(pu)) => Ok(pu),
        (Some(kv), Some(pu)) => Err(format!(
            "the voltage-dependent load at bus {bus} states a nominal voltage of {stated}, \
             which reads as {kv:.4} pu in kV against the bus base of {base_kv} kV and as \
             {pu:.4} pu in per unit; state it unambiguously (or leave it out for the bus base \
             voltage)"
        )),
        (None, None) => Err(format!(
            "the voltage-dependent load at bus {bus} states a nominal voltage of {stated}, \
             plausible neither in kV against the bus base of {base_kv} kV nor in per unit"
        )),
    }
}

/// Resolve load `index` of `network` into its voltage-dependent terms in the AC
/// model's per-unit system (`bus` left for the caller to set), or `None` for a
/// constant-power load. `scale` converts the load's unit to per unit on the
/// system base (`1 / base_mva`, or 1 for a normalized network). A ZIP
/// constant-power part and an exponential `γ = 0` side add no term: the stated
/// `p`/`q` less the terms' nominal powers is the load's constant-power part.
///
/// # Errors
/// A non-finite coefficient or exponent, or an unusable nominal voltage.
pub(crate) fn resolve(
    network: &BalancedNetwork,
    index: usize,
    base_kv: f64,
    scale: f64,
) -> Result<Option<Vec<LoadTerm>>, String> {
    let load = &network.loads()[index];
    let bus = load.bus.0;
    let finite = |values: &[f64]| -> Result<(), String> {
        if values.iter().all(|v| v.is_finite()) {
            Ok(())
        } else {
            Err(format!(
                "the voltage-dependent load at bus {bus} states a non-finite coefficient"
            ))
        }
    };
    let term = |reactive: bool, coefficient: f64, v_nom: f64, exponent: f64| LoadTerm {
        bus: usize::MAX,
        reactive,
        coefficient: coefficient * scale,
        v_nom,
        exponent,
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
            finite(&[
                load.p,
                load.q,
                *p_constant_power,
                *q_constant_power,
                *p_constant_current,
                *q_constant_current,
                *p_constant_impedance,
                *q_constant_impedance,
            ])?;
            let vn = nominal_pu(*v_nom, base_kv, bus)?;
            Ok(Some(
                [
                    (false, *p_constant_current, 1.0),
                    (true, *q_constant_current, 1.0),
                    (false, *p_constant_impedance, 2.0),
                    (true, *q_constant_impedance, 2.0),
                ]
                .into_iter()
                .filter(|&(_, c, _)| c != 0.0)
                .map(|(reactive, c, g)| term(reactive, c, vn, g))
                .collect(),
            ))
        }
        Some(LoadVoltageModel::Exponential {
            p,
            q,
            v_nom,
            gamma_p,
            gamma_q,
        }) => {
            finite(&[load.p, load.q, *p, *q, *gamma_p, *gamma_q])?;
            let vn = nominal_pu(*v_nom, base_kv, bus)?;
            // γ = 0 is constant power: it stays in the stated demand.
            Ok(Some(
                [(false, *p, *gamma_p), (true, *q, *gamma_q)]
                    .into_iter()
                    .filter(|&(_, c, g)| c != 0.0 && g != 0.0)
                    .map(|(reactive, c, g)| term(reactive, c, vn, g))
                    .collect(),
            ))
        }
        _ => Ok(None),
    }
}

/// Per dense bus, the change to the prepared demand `(pd, qd)`; and the
/// voltage-dependent terms.
pub(crate) type Attached = (BTreeMap<usize, (f64, f64)>, Vec<LoadTerm>);

/// Resolve every in-service voltage-dependent load of `network` onto the dense
/// buses `dense` names (source bus id → dense index). Returns, per dense bus,
/// the change to apply to the prepared demand `(pd, qd)` (which aggregates each
/// load's stated `p`/`q`), minus the nominal power of every voltage-dependent
/// term, and the terms themselves. The stated `p`/`q` stays authoritative for
/// the rest: whatever it holds beyond the terms (the ZIP constant-power part,
/// an exponential `γ = 0` side, or a demand edit written onto `p`) is constant
/// power, so an edited `p` is never silently dropped.
/// A load at a bus the energizing step isolated is left out with its bus.
///
/// # Errors
/// As [`resolve`], or an in-service voltage-dependent load at an energized bus
/// the model has no index for.
pub(crate) fn attach(
    network: &BalancedNetwork,
    dense: &BTreeMap<usize, usize>,
    scale: f64,
) -> Result<Attached, String> {
    let buses: BTreeMap<usize, (f64, BusType)> = network
        .buses()
        .iter()
        .map(|b| (b.id.0, (b.base_kv, b.kind)))
        .collect();
    let mut shift: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    let mut terms = Vec::new();
    for index in voltage_dependent(network) {
        let id = network.loads()[index].bus.0;
        let (base_kv, kind) = buses.get(&id).copied().unwrap_or((0.0, BusType::Isolated));
        let Some(&bus) = dense.get(&id) else {
            if kind == BusType::Isolated {
                continue;
            }
            return Err(format!(
                "the voltage-dependent load at bus {id} has no bus in the AC model"
            ));
        };
        let Some(resolved) = resolve(network, index, base_kv, scale)? else {
            continue;
        };
        let entry = shift.entry(bus).or_insert((0.0, 0.0));
        for t in &resolved {
            if t.reactive {
                entry.1 -= t.coefficient;
            } else {
                entry.0 -= t.coefficient;
            }
        }
        terms.extend(resolved.into_iter().map(|t| LoadTerm { bus, ..t }));
    }
    Ok((shift, terms))
}

/// What a solve with voltage-dependent loads reports about them.
pub(crate) fn diagnostic(network: &BalancedNetwork) -> Option<SolveDiagnostic> {
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
             voltage: the Newton iteration evaluates them at each iterate with their exact \
             voltage derivative, and the AC sensitivities include that dependence (a demand \
             parameter or edit changes the constant-power part)",
            dependent.len()
        ),
        buses,
    })
}

/// A ZIP model with active parts `p = [constant power, current, impedance]` and
/// reactive parts `q`, in the load's unit.
#[cfg(test)]
pub(crate) fn zip(p: [f64; 3], q: [f64; 3], v_nom: Option<f64>) -> LoadVoltageModel {
    LoadVoltageModel::Zip {
        p_constant_power: p[0],
        q_constant_power: q[0],
        p_constant_current: p[1],
        q_constant_current: q[1],
        p_constant_impedance: p[2],
        q_constant_impedance: q[2],
        v_nom,
        load_type: None,
        scaling: None,
    }
}

/// An exponential model, in the load's unit.
#[cfg(test)]
pub(crate) fn exponential(
    p: f64,
    q: f64,
    gamma_p: f64,
    gamma_q: f64,
    v_nom: Option<f64>,
) -> LoadVoltageModel {
    LoadVoltageModel::Exponential {
        p,
        q,
        v_nom,
        gamma_p,
        gamma_q,
    }
}

#[cfg(test)]
#[path = "load_voltage_tests.rs"]
mod tests;
