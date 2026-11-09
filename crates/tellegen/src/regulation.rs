//! Explicit steady-state outer-loop regulation. Source networks are never mutated.
//! Discrete settings are selected from their admissible grid; continuous settings
//! use a safeguarded local response estimate. No unbounded regulator is invented.
use crate::api::{Problem, SolveRequest, SolveResponse};
use powerio::{BalancedNetwork, TransformerControlMode as Mode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub(crate) enum Setting {
    Tap(usize),
    Phase(usize),
    Shunt(usize),
    ReactiveLoad(usize),
}
#[derive(Clone, Debug)]
pub(crate) enum Measurement {
    Voltage(usize),
    ActiveFlow(usize),
    ReactiveFlow(usize),
}
#[derive(Clone, Debug)]
pub(crate) struct Control {
    pub id: String,
    pub setting: Setting,
    pub measurement: Measurement,
    pub low: f64,
    pub high: f64,
    pub min: f64,
    pub max: f64,
    pub positions: Vec<f64>,
    pub units: &'static str,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlResult {
    pub id: String,
    pub setting: f64,
    pub units: String,
    pub measured: f64,
    pub measured_units: String,
    pub target_low: f64,
    pub target_high: f64,
    pub status: String,
}
#[derive(Debug, Serialize)]
pub struct RegulatedSolution {
    pub solution: SolveResponse,
    pub controls: Vec<ControlResult>,
    pub passes: usize,
    pub violations: Vec<String>,
}
fn reading(c: &Control, s: &SolveResponse) -> Result<f64, String> {
    match c.measurement {
        Measurement::Voltage(bus) => {
            s.vm.as_ref()
                .and_then(|v| v.iter().find(|v| v.bus == bus))
                .map(|v| v.value)
        }
        Measurement::ActiveFlow(branch) => s
            .flows
            .as_ref()
            .and_then(|v| v.iter().find(|v| v.branch == branch))
            .map(|v| v.pf),
        Measurement::ReactiveFlow(branch) => s
            .flows
            .as_ref()
            .and_then(|v| v.iter().find(|v| v.branch == branch))
            .and_then(|v| v.qf),
    }
    .filter(|v| v.is_finite())
    .ok_or_else(|| format!("{} has no solved measurement", c.id))
}
fn setting(c: &Control, n: &BalancedNetwork) -> f64 {
    match c.setting {
        Setting::Tap(i) => n.branches()[i].tap,
        Setting::Phase(i) => n.branches()[i].shift,
        Setting::Shunt(i) => n.shunts()[i].b,
        Setting::ReactiveLoad(i) => n.loads()[i].q,
    }
}
fn assign(c: &Control, n: &mut BalancedNetwork, v: f64) {
    match c.setting {
        Setting::Tap(i) => n.branches_mut()[i].tap = v,
        Setting::Phase(i) => n.branches_mut()[i].shift = v,
        Setting::Shunt(i) => n.shunts_mut()[i].b = v,
        Setting::ReactiveLoad(i) => n.loads_mut()[i].q = v,
    }
}
fn error(c: &Control, y: f64) -> f64 {
    (c.low - y).max(y - c.high).max(0.0)
}
fn solve(n: &BalancedNetwork) -> Result<SolveResponse, String> {
    crate::api::solve_network(
        n,
        &SolveRequest {
            formulation: Problem::AcPf,
            ..Default::default()
        },
    )
}

/// Seed candidate power flows with the last accepted operating point. Numerical
/// Jacobians are rebuilt because tap/shunt actions change the admittance matrix.
fn solve_from(n: &mut BalancedNetwork, previous: &SolveResponse) -> Result<SolveResponse, String> {
    let normalized = n.is_normalized();
    let vm: std::collections::HashMap<_, _> = previous
        .vm
        .iter()
        .flatten()
        .map(|v| (v.bus, v.value))
        .collect();
    let va: std::collections::HashMap<_, _> = previous
        .va
        .iter()
        .flatten()
        .map(|v| (v.bus, v.value))
        .collect();
    for b in n.buses_mut() {
        if let Some(&v) = vm.get(&b.id.0) {
            b.vm = v;
        }
        if let Some(&v) = va.get(&b.id.0) {
            b.va = if normalized { v } else { v.to_degrees() };
        }
    }
    crate::api::solve_network(
        n,
        &SolveRequest {
            formulation: Problem::AcPf,
            start: crate::api::PowerFlowStart::Case,
            ..Default::default()
        },
    )
}

pub(crate) fn source_controls(n: &BalancedNetwork) -> Result<Vec<Control>, String> {
    let mut out = Vec::new();
    if n.detailed_connectivity()
        .as_ref()
        .is_some_and(|d| d.tap_changers.iter().any(|t| t.regulating))
    {
        return Err("Automatic regulation of detailed tap tables requires per-position impedance and winding-side semantics; use an explicit uniform transformer control".into());
    }
    for (i, b) in n.branches().iter().enumerate() {
        let Some(c) = b.control.as_ref().filter(|c| c.enabled) else {
            continue;
        };
        if !b.in_service || c.mode == Mode::Fixed {
            continue;
        }
        if c.regulating_terminal.is_some()
            || c.controlled_bus_on_winding_side
            || b.impedance_correction().is_some()
        {
            return Err(format!(
                "Transformer {} needs a terminal/tap-table-specific regulator model",
                i + 1
            ));
        }
        let (s, m, units) = match c.mode {
            Mode::Voltage => (
                Setting::Tap(i),
                Measurement::Voltage(c.controlled_bus.unwrap_or(b.to).0),
                "ratio",
            ),
            Mode::ReactiveFlow => (Setting::Tap(i), Measurement::ReactiveFlow(i + 1), "ratio"),
            Mode::ActiveFlow => (
                Setting::Phase(i),
                Measurement::ActiveFlow(i + 1),
                if n.is_normalized() { "rad" } else { "degrees" },
            ),
            _ => {
                return Err(format!(
                    "Transformer {} has an unsupported automatic control mode",
                    i + 1
                ))
            }
        };
        if !matches!(c.mode, Mode::Voltage) && c.controlled_bus.is_some_and(|bus| bus != b.from) {
            return Err("Flow regulation requires an explicit from-terminal target".into());
        }
        if matches!(c.mode, Mode::Voltage | Mode::ReactiveFlow) && c.tap_min <= 0.0 {
            return Err("Automatic ratio control requires strictly positive tap ratios".into());
        }
        if c.ntp < 2 || c.ntp > 1000 {
            return Err("Automatic transformer control needs 2–1000 stated tap positions".into());
        }
        let p = (0..c.ntp)
            .map(|j| c.tap_min + (c.tap_max - c.tap_min) * f64::from(j) / f64::from(c.ntp - 1))
            .collect();
        out.push(Control {
            id: format!("transformer:{}", i + 1),
            setting: s,
            measurement: m,
            low: c.band_min
                * if n.is_normalized() && c.mode != Mode::Voltage {
                    n.base_mva()
                } else {
                    1.0
                },
            high: c.band_max
                * if n.is_normalized() && c.mode != Mode::Voltage {
                    n.base_mva()
                } else {
                    1.0
                },
            min: c.tap_min,
            max: c.tap_max,
            positions: p,
            units,
        });
    }
    for (i, s) in n.shunts().iter().enumerate() {
        use powerio_tx::SwitchedShuntMode as ShuntMode;
        let Some(c) = s.control.as_ref().filter(|_| s.in_service) else {
            continue;
        };
        if c.mode == ShuntMode::Locked {
            continue;
        }
        if s.extras
            .get("psse_modsw")
            .and_then(|v| v.as_i64())
            .is_some_and(|v| v > 2)
        {
            return Err("Non-voltage PSS/E shunt regulation is not a voltage controller".into());
        }
        if c.regulating_terminal.is_some()
            || (c.rmpct - 100.0).abs() > 1e-9
            || s.g != 0.0
            || c.blocks.iter().any(|b| b.g != 0.0)
        {
            return Err("Automatic shunt control requires a pure susceptance bank with full local participation".into());
        }
        if c.blocks.iter().any(|b| b.b > 0.0) && c.blocks.iter().any(|b| b.b < 0.0) {
            return Err("Mixed-sign shunt blocks require an explicit switching sequence".into());
        }
        let mut p = vec![0.0];
        for block in &c.blocks {
            if block.steps > 1000 || p.len() + block.steps as usize > 1001 {
                return Err("Shunt step table exceeds 1000 sections".into());
            }
            for _ in 0..block.steps {
                p.push(p.last().unwrap() + block.b);
            }
        }
        p.sort_by(f64::total_cmp);
        p.dedup();
        if p.len() < 2 {
            return Err("Automatic shunt control needs stated nonzero blocks".into());
        }
        let (min, max) = (p[0], *p.last().unwrap());
        if c.mode == ShuntMode::Continuous {
            p.clear();
        } else if c.mode != ShuntMode::Discrete {
            return Err("Unsupported shunt control mode".into());
        }
        out.push(Control {
            id: format!("shunt:{}", i + 1),
            setting: Setting::Shunt(i),
            measurement: Measurement::Voltage(c.control_bus.unwrap_or(s.bus).0),
            low: c.vlow,
            high: c.vhigh,
            min,
            max,
            positions: p,
            units: if n.is_normalized() {
                "pu B"
            } else {
                "Mvar at 1 pu"
            },
        });
    }
    Ok(out)
}

pub(crate) fn run(
    mut n: BalancedNetwork,
    controls: Vec<Control>,
) -> Result<RegulatedSolution, String> {
    if controls.len() > 64 {
        return Err("Automatic regulation currently supports at most 64 controls".into());
    }
    for c in &controls {
        if ![c.min, c.max, c.low, c.high].iter().all(|v| v.is_finite())
            || c.min > c.max
            || c.low > c.high
        {
            return Err(format!(
                "{} has invalid setting bounds or target band",
                c.id
            ));
        }
        let initial = setting(c, &n);
        if !initial.is_finite() || initial < c.min - 1e-9 || initial > c.max + 1e-9 {
            return Err(format!("{} initial setting is outside its bounds", c.id));
        }
        if !c.positions.is_empty() && !c.positions.iter().any(|v| (v - initial).abs() < 1e-7) {
            return Err(format!(
                "{} initial setting is not a declared discrete position",
                c.id
            ));
        }
    }
    let mut s = solve(&n)?;
    let mut seen = std::collections::HashSet::new();
    // A monotone traversal of every declared setting needs at most the sum
    // of discrete moves, plus the final no-action pass. Continuous controls
    // receive the original 60-pass allowance. Cycling is still rejected.
    let max_passes = 60
        + controls
            .iter()
            .map(|c| c.positions.len().saturating_sub(1))
            .sum::<usize>();
    for pass in 0..max_passes {
        let signature: Vec<_> = controls.iter().map(|c| setting(c, &n).to_bits()).collect();
        if !seen.insert(signature) {
            return Err("Automatic regulators cycle; check competing targets and deadbands".into());
        }
        let mut changed = false;
        for c in &controls {
            let y = reading(c, &s)?;
            let old = setting(c, &n);
            let tol = if matches!(c.measurement, Measurement::Voltage(_)) {
                1e-6
            } else {
                1e-4
            };
            if error(c, y) <= tol || c.min == c.max {
                continue;
            }
            let mut candidates = Vec::new();
            if !c.positions.is_empty() {
                // Adjacent actions preserve the declared sequence; never round a relaxed optimum.
                if let Some(&v) = c.positions.iter().rev().find(|&&v| v < old - 1e-9) {
                    candidates.push(v)
                }
                if let Some(&v) = c.positions.iter().find(|&&v| v > old + 1e-9) {
                    candidates.push(v)
                }
            } else {
                let h = ((c.max - c.min) * 1e-4)
                    .max(1e-6)
                    .min((c.max - c.min) * 0.5);
                let probe = if old + h <= c.max { old + h } else { old - h };
                let mut test = n.clone();
                assign(c, &mut test, probe);
                let ys = reading(c, &solve_from(&mut test, &s)?)?;
                let slope = (ys - y) / (probe - old);
                if !slope.is_finite() || slope.abs() < 1e-12 {
                    return Err(format!("{} has no usable control response", c.id));
                }
                let target = ((c.low + c.high) * 0.5 - y) / slope;
                for fraction in [1.0, 0.5, 0.25, 0.125] {
                    candidates.push((old + fraction * target).clamp(c.min, c.max));
                }
            }
            let mut best = None;
            let mut best_error = error(c, y);
            for value in candidates {
                let mut test = n.clone();
                assign(c, &mut test, value);
                if let Ok(next) = solve_from(&mut test, &s) {
                    let e = error(c, reading(c, &next)?);
                    if e < best_error - 1e-10 {
                        best_error = e;
                        best = Some((test, next));
                    }
                }
            }
            if let Some((next, result)) = best {
                n = next;
                s = result;
                changed = true;
            }
        }
        if !changed {
            let results = controls
                .iter()
                .map(|c| {
                    let v = setting(c, &n);
                    let y = reading(c, &s)?;
                    let tol = if matches!(c.measurement, Measurement::Voltage(_)) {
                        1e-6
                    } else {
                        1e-4
                    };
                    let status = if error(c, y) <= tol {
                        "regulated"
                    } else if (v - c.min).abs() < 1e-8 || (v - c.max).abs() < 1e-8 {
                        "at_limit"
                    } else if !c.positions.is_empty() {
                        "discrete_deadband_unreachable"
                    } else {
                        "unresolved"
                    };
                    if status == "unresolved" {
                        return Err(format!("{} did not reach its target", c.id));
                    }
                    Ok(ControlResult {
                        id: c.id.clone(),
                        setting: v,
                        units: c.units.into(),
                        measured: y,
                        measured_units: match c.measurement {
                            Measurement::Voltage(_) => "pu",
                            Measurement::ActiveFlow(_) => "MW",
                            Measurement::ReactiveFlow(_) => "Mvar",
                        }
                        .into(),
                        target_low: c.low,
                        target_high: c.high,
                        status: status.into(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(RegulatedSolution {
                violations: operating_violations(&n, &s),
                solution: s,
                controls: results,
                passes: pass + 1,
            });
        }
    }
    Err(format!(
        "Automatic regulation exhausted its {max_passes}-pass budget"
    ))
}

/// Numerical convergence is separate from the stated operating limits.
pub(crate) fn operating_violations(n: &BalancedNetwork, s: &SolveResponse) -> Vec<String> {
    let mut out = Vec::new();
    for v in s.vm.iter().flatten() {
        if let Some(b) = n.buses().iter().find(|b| b.id.0 == v.bus) {
            if v.value < b.vmin - 1e-7 || v.value > b.vmax + 1e-7 {
                out.push(format!("AC bus {} exceeds its voltage range", v.bus));
            }
        }
    }
    for f in s.flows.iter().flatten() {
        if f.loading > 1.0 + 1e-7 {
            out.push(format!(
                "AC branch {} exceeds its reported thermal rating",
                f.branch
            ));
        }
    }
    let scale = if n.is_normalized() { n.base_mva() } else { 1.0 };
    for g in s.dispatch.iter().flatten() {
        if let Some(source) = g
            .gen
            .checked_sub(1)
            .and_then(|i| n.generators().get(i))
            .filter(|g| g.in_service)
        {
            if g.pg < source.pmin * scale - 1e-6 || g.pg > source.pmax * scale + 1e-6 {
                out.push(format!(
                    "AC generator {} exceeds active power limits",
                    g.gen
                ));
            }
            if g.qg
                .is_some_and(|q| q < source.qmin * scale - 1e-6 || q > source.qmax * scale + 1e-6)
            {
                out.push(format!(
                    "AC generator {} exceeds reactive power limits",
                    g.gen
                ));
            }
        }
    }
    out
}

pub fn solve_source(n: &BalancedNetwork) -> Result<RegulatedSolution, String> {
    run(n.clone(), source_controls(n)?)
}
pub fn solve_module_json(input: &str) -> Result<String, String> {
    let n = crate::ir::balanced_module(crate::ir::deserialize_module(input)?)?.into_value();
    serde_json::to_string(&solve_source(&n)?).map_err(|e| e.to_string())
}

#[cfg(all(test, feature = "sensitivity"))]
mod tests {
    use super::*;
    fn fixture() -> BalancedNetwork {
        let mut n = crate::dc_grid::tests::imported_fixture();
        *n.detailed_connectivity_mut() = None;
        n
    }
    #[test]
    fn large_shunt_reaches_a_target_more_than_sixty_steps_away() {
        let mut n = fixture();
        for b in n.branches_mut() {
            b.control = None;
        }
        for s in n.shunts_mut() {
            s.control = None;
        }
        let row = n.shunts().len();
        n.shunts_mut()
            .push(powerio::Shunt::new(powerio::BusId(2), 0.0, 0.0));
        let mut target = n.clone();
        target.shunts_mut()[row].b = 20.0;
        let y = solve(&target)
            .unwrap()
            .vm
            .unwrap()
            .into_iter()
            .find(|b| b.bus == 2)
            .unwrap()
            .value;
        let c = Control {
            id: "large-bank".into(),
            setting: Setting::Shunt(row),
            measurement: Measurement::Voltage(2),
            low: y - 1e-7,
            high: y + 1e-7,
            min: 0.0,
            max: 25.0,
            positions: (0..=1000).map(|i| f64::from(i) * 0.025).collect(),
            units: "Mvar at 1 pu",
        };
        let before = serde_json::to_string(&n).unwrap();
        let start = std::time::Instant::now();
        let r = run(n.clone(), vec![c]).unwrap();
        eprintln!("large-bank: {} passes in {:?}", r.passes, start.elapsed());
        assert!(r.passes > 60);
        assert_eq!(r.controls[0].status, "regulated");
        assert_eq!(serde_json::to_string(&n).unwrap(), before);
    }

    #[test]
    fn conflicting_discrete_controls_still_report_a_cycle() {
        let mut n = fixture();
        let row = n.shunts().len();
        n.shunts_mut()
            .push(powerio::Shunt::new(powerio::BusId(2), 0.0, 0.0));
        let low = solve(&n)
            .unwrap()
            .vm
            .unwrap()
            .into_iter()
            .find(|v| v.bus == 2)
            .unwrap()
            .value;
        let mut target = n.clone();
        target.shunts_mut()[row].b = 20.0;
        let high = solve(&target)
            .unwrap()
            .vm
            .unwrap()
            .into_iter()
            .find(|v| v.bus == 2)
            .unwrap()
            .value;
        let c = Control {
            id: "raise".into(),
            setting: Setting::Shunt(row),
            measurement: Measurement::Voltage(2),
            low: high - 1e-6,
            high: high + 1e-6,
            min: 0.0,
            max: 20.0,
            positions: vec![0.0, 20.0],
            units: "Mvar at 1 pu",
        };
        let mut other = c.clone();
        other.id = "lower".into();
        other.low = low - 1e-6;
        other.high = low + 1e-6;
        assert!(run(n, vec![c, other]).unwrap_err().contains("cycle"));
    }

    #[test]
    fn warm_start_preserves_native_and_normalized_solutions() {
        let raw = fixture();
        for mut n in [raw.clone(), raw.to_normalized().unwrap()] {
            let cold = solve(&n).unwrap();
            let warm = solve_from(&mut n, &cold).unwrap();
            for (a, b) in cold.vm.unwrap().iter().zip(warm.vm.unwrap()) {
                assert!((a.value - b.value).abs() < 1e-7);
            }
            for (a, b) in cold.va.unwrap().iter().zip(warm.va.unwrap()) {
                assert!((a.value - b.value).abs() < 1e-7);
            }
        }
    }

    #[test]
    fn discrete_tap_reaches_band_and_preserves_source() {
        let mut n = fixture();
        n.branches_mut()[0].tap = 1.0;
        let y = solve(&n)
            .unwrap()
            .vm
            .unwrap()
            .into_iter()
            .find(|v| v.bus == 2)
            .unwrap()
            .value;
        let mut c = powerio::TransformerControl::new(Mode::Voltage);
        c.controlled_bus = Some(powerio::BusId(2));
        c.tap_min = 0.8;
        c.tap_max = 1.2;
        c.ntp = 41;
        c.band_min = y + 0.01;
        c.band_max = y + 0.02;
        n.branches_mut()[0].control = Some(c);
        let before = serde_json::to_string(&n).unwrap();
        let result = solve_source(&n).unwrap();
        assert_eq!(result.controls[0].status, "regulated");
        assert_ne!(result.controls[0].setting, 1.0);
        assert!(
            ((result.controls[0].setting - 0.8) / 0.01).fract().abs() < 1e-6
                || ((result.controls[0].setting - 0.8) / 0.01).fract().abs() > 0.999999
        );
        assert_eq!(serde_json::to_string(&n).unwrap(), before);
    }
    #[test]
    fn shunt_regulates_and_reports_saturation() {
        let mut n = fixture();
        let y = solve(&n)
            .unwrap()
            .vm
            .unwrap()
            .into_iter()
            .find(|v| v.bus == 2)
            .unwrap()
            .value;
        let mut s = powerio::Shunt::new(powerio::BusId(2), 0.0, 0.0);
        let blocks =
            serde_json::from_value(serde_json::json!([{"steps":10,"b":5.0,"g":0.0}])).unwrap();
        s.control = Some(powerio_tx::SwitchedShuntControl::new(
            powerio_tx::SwitchedShuntMode::Discrete,
            y + 0.02,
            y + 0.01,
            blocks,
        ));
        n.shunts_mut().push(s);
        let r = solve_source(&n).unwrap();
        assert_eq!(r.controls.last().unwrap().status, "regulated");
        n.shunts_mut()
            .last_mut()
            .unwrap()
            .control
            .as_mut()
            .unwrap()
            .vlow = 2.0;
        n.shunts_mut()
            .last_mut()
            .unwrap()
            .control
            .as_mut()
            .unwrap()
            .vhigh = 2.01;
        let r = solve_source(&n).unwrap();
        assert_eq!(r.controls.last().unwrap().status, "at_limit");
        assert_eq!(r.controls.last().unwrap().setting, 50.0);
    }
    #[test]
    fn phase_flow_control_has_native_and_normalized_parity() {
        let mut n = fixture();
        for b in n.branches_mut() {
            b.control = None;
            b.in_service = true;
        }
        for s in n.shunts_mut() {
            s.control = None;
        }
        n.branches_mut()[1].tap = 1.0;
        n.branches_mut()[1].shift = 0.0;
        let flow = solve(&n)
            .unwrap()
            .flows
            .unwrap()
            .into_iter()
            .find(|f| f.branch == 2)
            .unwrap()
            .pf;
        let mut c = powerio::TransformerControl::new(Mode::ActiveFlow);
        c.tap_min = -20.0;
        c.tap_max = 20.0;
        c.ntp = 81;
        c.band_min = flow - 30.0;
        c.band_max = flow - 2.0;
        n.branches_mut()[1].control = Some(c);
        let raw = solve_source(&n).unwrap();
        assert_eq!(raw.controls[0].status, "regulated");
        let normalized = n
            .to_normalized_with_source_rows(&powerio_tx::NormalizeOptions::default())
            .unwrap()
            .0
            .network;
        let norm = solve_source(&normalized).unwrap();
        assert!((raw.controls[0].setting.to_radians() - norm.controls[0].setting).abs() < 1e-10);
        assert!((raw.controls[0].measured - norm.controls[0].measured).abs() < 1e-6);
    }
}
