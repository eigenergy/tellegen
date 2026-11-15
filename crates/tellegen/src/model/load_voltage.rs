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
//! writes. When a source states one, PowerIO's `calc_load_nominal_voltage`
//! reads it against the bus base voltage. Every term, its derivative and the
//! nominal voltage are PowerIO's (`powerio_tx::load_voltage`); this module
//! places them on the model's buses.

use std::collections::BTreeMap;

use powerio::{BalancedNetwork, BusType, LoadVoltageModel};

use crate::api::SolveDiagnostic;

/// PowerIO's ratio floor (see [`LoadTerm::eval`]). A converged point with any
/// voltage-dependent term below the floor is refused rather than reported:
/// for `γ < 1` it was solved on the continuation, and for `γ ≥ 1` it is the
/// collapsed root `vm = 0` that such a load admits at any loading.
#[cfg(test)]
pub(crate) const RATIO_FLOOR: f64 = powerio_tx::LOAD_VOLTAGE_RATIO_FLOOR;

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
    /// PowerIO's term of the same parts.
    fn powerio(&self) -> powerio_tx::LoadVoltageTerm {
        powerio_tx::LoadVoltageTerm::new(self.reactive, self.coefficient, self.v_nom, self.exponent)
    }

    /// The term's power at magnitude `vm` (per unit) and its derivative w.r.t. `vm`:
    /// PowerIO's `LoadVoltageTerm::calc_power`, `(c x^γ, γ c x^(γ−1) / v_nom)` with
    /// `x = vm / v_nom`, continued by its tangent below the ratio floor for `γ < 1`.
    pub(crate) fn eval(&self, vm: f64) -> (f64, f64) {
        self.powerio().calc_power(vm)
    }

    /// Whether `vm` lies below the term's ratio floor: a collapsed voltage, not an
    /// operating point (PowerIO's `LoadVoltageTerm::is_below_floor`).
    pub(crate) fn below_floor(&self, vm: f64) -> bool {
        self.powerio().is_below_floor(vm)
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

/// A load's nominal voltage in per unit on its bus base: PowerIO's
/// `calc_load_nominal_voltage`, its refusal worded for bus `bus`.
///
/// # Errors
/// A nominal voltage PowerIO refuses.
#[cfg(test)]
pub(crate) fn nominal_pu(v_nom: Option<f64>, base_kv: f64, bus: usize) -> Result<f64, String> {
    powerio_tx::calc_load_nominal_voltage(v_nom, base_kv)
        .map_err(|error| format!("the voltage-dependent load at bus {bus} {error}"))
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
    let terms = load
        .calc_voltage_terms(base_kv)
        .map_err(|error| format!("the voltage-dependent load at bus {bus} {error}"))?;
    Ok(terms.map(|terms| {
        terms
            .into_iter()
            .map(|term| LoadTerm {
                bus: usize::MAX,
                reactive: term.reactive,
                coefficient: term.coefficient * scale,
                v_nom: term.v_nom,
                exponent: term.exponent,
            })
            .collect()
    }))
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
///
/// Only loads on buses the solved model contains (`dense`: bus id to model
/// index) are counted; a load on a de-energized island draws nothing.
pub(crate) fn diagnostic(
    network: &BalancedNetwork,
    dense: &BTreeMap<usize, usize>,
) -> Option<SolveDiagnostic> {
    let dependent: Vec<usize> = voltage_dependent(network)
        .into_iter()
        .filter(|&i| dense.contains_key(&network.loads()[i].bus.0))
        .collect();
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
