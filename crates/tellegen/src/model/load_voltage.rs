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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Problem, Study};

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/case_data.rs"
    ));

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/smib.rs"
    ));

    /// A single-machine case from WECC 240 with a ZIP load at the unit's bus.
    fn smib_with_zip_load() -> (BalancedNetwork, powerio::DynamicCase) {
        let case = &wecc240_smib_cases()[0];
        let raw = with_load(&case.raw, "102,'1 ',1,   1,   1,    40.000,    10.000,    30.000,     5.000,    20.000,     4.000,   1,1");
        let src = powerio::Source::from_memory("case.raw", raw.into_bytes()).unwrap();
        let module = powerio::parse(src).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        (net.clone(), powerio::parse_dyr(&case.dyr).unwrap())
    }

    #[test]
    fn a_zip_load_settles_at_its_power_at_the_solved_voltage() {
        let (net, _) = smib_with_zip_load();
        assert_eq!(voltage_dependent(&net).len(), 1);
        // A stand-in for a power flow: the bus voltage falls with the load.
        let mut seen = Vec::new();
        let ((), rounds) = settle(&net, |n| {
            let p: f64 = n.loads().iter().map(|l| l.p).sum();
            seen.push(p);
            Ok(((), vec![(102, 1.05 - 0.001 * p)]))
        })
        .unwrap();
        assert!(rounds > 1);
        // At the fixed point the load draws its ZIP power at the voltage it
        // produced.
        let Some(LoadVoltageModel::Zip {
            p_constant_power: a,
            p_constant_current: b,
            p_constant_impedance: c,
            ..
        }) = net.loads()[0].voltage_model.clone()
        else {
            panic!("not a ZIP load")
        };
        let p = *seen.last().unwrap();
        let v = 1.05 - 0.001 * p;
        let expected = a + b * v + c * v * v;
        assert!((p - expected).abs() < 1e-6, "{p} vs {expected}");
    }

    #[test]
    fn a_nominal_voltage_in_kv_is_taken_against_the_bus_base() {
        let (net, _) = smib_with_zip_load();
        let mut load = net.loads()[0].clone();
        let Some(LoadVoltageModel::Zip { v_nom, .. }) = load.voltage_model.as_mut() else {
            panic!("not a ZIP load")
        };
        *v_nom = Some(230.0);
        let v = BTreeMap::from([(102, 0.9)]);
        let kv = BTreeMap::from([(102, 230.0)]);
        let in_kv = power_at(&load, &v, &kv).unwrap();
        let Some(LoadVoltageModel::Zip { v_nom, .. }) = load.voltage_model.as_mut() else {
            unreachable!()
        };
        *v_nom = Some(1.0);
        assert_eq!(in_kv, power_at(&load, &v, &kv).unwrap());
        assert!(power_at(&load, &v, &BTreeMap::new()).is_ok());
        let Some(LoadVoltageModel::Zip { v_nom, .. }) = load.voltage_model.as_mut() else {
            unreachable!()
        };
        *v_nom = Some(230.0);
        assert!(power_at(&load, &v, &BTreeMap::new()).is_err());
    }

    fn module(case: &str) -> String {
        let dir = rms_example(case).expect("case data");
        let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
        crate::ir::serialize_module(&module).unwrap()
    }

    /// The study's AC power flow of a case with voltage-dependent loads
    /// reports them and finds the operating point the dynamic initialization
    /// starts from.
    fn matches_dynamics(json: &str, dependent: usize) {
        let study = Study::new(json, Problem::AcPf).expect("acpf with voltage-dependent loads");
        let response = study.solution();
        let note = response
            .diagnostics
            .iter()
            .find(|d| d.code == "load_voltage_dependent")
            .expect("the voltage-dependent loads are reported");
        assert!(
            note.message.starts_with(&format!("{dependent} ")),
            "{}",
            note.message
        );
        let network = crate::ir::balanced_module(crate::ir::deserialize_module(json).unwrap())
            .unwrap()
            .value()
            .clone();
        assert_eq!(voltage_dependent(&network).len(), dependent);
        let (ac, pf, rounds) = crate::rms::init::power_flow(&network).unwrap();
        assert!(rounds > 1, "{rounds} rounds");
        let vm = response.vm.as_ref().unwrap();
        let worst = ac
            .bus_ids
            .iter()
            .zip(&pf.vm)
            .map(|(id, v)| {
                let s = vm.iter().find(|b| b.bus == *id).expect("bus in both").value;
                (s - v).abs()
            })
            .fold(0.0_f64, f64::max);
        assert!(worst < 1e-8, "|V| differs by {worst:e}");
    }

    /// A ZIP load at the unit's bus, the unit's reactive limits wide (the RMS
    /// initialization holds generators at their setpoints, the static power
    /// flow applies the limits): both find the same operating point.
    #[test]
    fn ac_power_flow_with_a_zip_load() {
        let case = &wecc240_smib_cases()[0];
        let raw = with_load(&case.raw, "102,'1 ',1,   1,   1,    40.000,    10.000,    30.000,     5.000,    20.000,     4.000,   1,1");
        let raw: String = raw
            .lines()
            .map(|l| {
                if l.starts_with("   102,'1 ',") && l.split(',').count() > 18 {
                    let mut f: Vec<String> = l.split(',').map(str::to_owned).collect();
                    f[4] = " 9999.000".into();
                    f[5] = " -9999.000".into();
                    f.join(",")
                } else {
                    l.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let src = powerio::Source::from_memory("case.raw", raw.into_bytes()).unwrap();
        let json = crate::ir::serialize_module(&powerio::parse(src).unwrap()).unwrap();
        matches_dynamics(&json, 1);
    }

    /// WECC 240's ZIP loads no longer stop the AC power flow; what remains is
    /// its generators' remote voltage control.
    #[test]
    fn wecc240_stops_only_at_remote_voltage_control() {
        let Err(error) = Study::new(&module("wecc240"), Problem::AcPf) else {
            panic!("remote voltage control is still refused");
        };
        assert!(error.contains("regulating a remote bus"), "{error}");
    }

    #[test]
    fn nordic44_ac_power_flow_with_zip_loads() {
        if rms_example("nordic44").is_none() {
            return; // data kept outside the repository
        }
        matches_dynamics(&module("nordic44"), 13);
    }

    /// Without voltage-dependent loads the study solves once, as before.
    #[test]
    fn constant_power_cases_solve_once() {
        let case = &wecc240_smib_cases()[0];
        let src = powerio::Source::from_memory("case.raw", case.raw.as_bytes().to_vec()).unwrap();
        let json = crate::ir::serialize_module(&powerio::parse(src).unwrap()).unwrap();
        let study = Study::new(&json, Problem::AcPf).expect("acpf");
        assert!(study
            .solution()
            .diagnostics
            .iter()
            .all(|d| d.code != "load_voltage_dependent"));
    }

    /// The RMS initialization starts from that operating point and holds it.
    #[test]
    fn rms_starts_at_rest_with_a_zip_load() {
        use crate::rms::{LoadModel, RmsRequest, RmsSimulation};
        let (net, dyr) = smib_with_zip_load();
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        let r = sim
            .run(&RmsRequest {
                t_end: 1.0,
                small_signal: false,
                ..RmsRequest::default()
            })
            .unwrap();
        assert!(
            r.notes.iter().any(|n| n.contains("voltage-dependent")),
            "{:?}",
            r.notes
        );
        let w = &r.channels["omega:102"];
        assert!(w.iter().all(|x| (x - 1.0).abs() < 1e-8), "not at rest");
    }
}
