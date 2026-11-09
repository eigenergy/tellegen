//! Qualified steady-state AC/DC power flow and optional joint nonlinear OPF.
//! Power flow solves resistive DC controls before AC controls; joint OPF places
//! converter transfers and generation in the same nonlinear problem.
use crate::{
    api::{Problem, SolveRequest, SolveResponse},
    dc_grid::{self, DcGridSolution},
};
use powerio::{BalancedNetwork, BusId, Load};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AcDcSolution {
    pub dc: DcGridSolution,
    pub ac: SolveResponse,
    /// Ratings are checked separately from numerical convergence.
    pub violations: Vec<String>,
    pub controls: Vec<crate::regulation::ControlResult>,
    pub converter_reactive: Vec<(String, f64)>,
    pub scope: &'static str,
}

/// Standard steady-state LCC approximation. Both rectifier and inverter absorb Q.
/// Firing-angle and calculated-power-factor models require additional commutation
/// parameters absent from this reduced model and must not be silently substituted.
pub(crate) fn lcc_power_factor(c: &powerio_tx::LineCommutatedConverter) -> Result<f64, String> {
    if c.reactive_model != Some(powerio_tx::LineCommutatedConverterReactiveModel::FixedPowerFactor)
    {
        return Err("LCC requires an explicit fixed-power-factor model; calculated commutation models require a different parameterization".into());
    }
    if c.target_alpha_degrees.is_some() || c.target_gamma_degrees.is_some() {
        return Err("LCC firing/extinction-angle control cannot be combined with a fixed-power-factor approximation".into());
    }
    if !matches!(
        c.operating_mode,
        Some(
            powerio_tx::LineCommutatedConverterOperatingMode::Rectifier
                | powerio_tx::LineCommutatedConverterOperatingMode::Inverter
        )
    ) {
        return Err("LCC rectifier/inverter operating mode must be explicit".into());
    }
    let pf = c.power_factor.ok_or("LCC power factor missing")?;
    if !pf.is_finite() || pf <= 0.0 || pf > 1.0 {
        return Err("LCC power factor must be in (0,1]".into());
    }
    Ok(pf)
}

/// Validate the AC voltage-controller identity and target shared by PF and OPF.
/// Capability evaluation remains formulation-specific: PF has a fixed P, while
/// joint OPF would need curve constraints depending on its decision variable P.
pub(crate) fn voltage_target(
    net: &BalancedNetwork,
    source: &powerio_tx::VoltageSourceConverter,
    ac_bus: usize,
    regulated_buses: &mut std::collections::HashSet<usize>,
) -> Result<f64, String> {
    if !regulated_buses.insert(ac_bus) {
        return Err("Multiple converter voltage controllers at one AC bus require an explicit reactive-sharing model".into());
    }
    let bus = net
        .buses()
        .iter()
        .find(|b| b.id.0 == ac_bus)
        .ok_or("Converter AC bus missing")?;
    if bus.kind != powerio::BusType::Pq
        || net
            .generators()
            .iter()
            .any(|g| g.in_service && g.bus == bus.id && g.voltage_regulation_on)
    {
        return Err(
            "Converter voltage control conflicts with an existing bus voltage controller".into(),
        );
    }
    let target = source
        .voltage_setpoint_kv
        .ok_or("Converter AC voltage target missing")?
        / bus.base_kv;
    if !bus.base_kv.is_finite() || bus.base_kv <= 0.0 || !target.is_finite() || target <= 0.0 {
        return Err("Invalid converter AC voltage target".into());
    }
    Ok(target)
}

/// Detailed converter P/Q are positive for withdrawal at the AC PCC. These
/// are not the injection-positive setpoints of reduced VscConverterStation.
/// Keep the capability curve's P coordinate and both Q bounds in that same
/// source convention; in particular, do not negate only the setpoint.
pub(crate) fn reactive_bounds(
    limits: &powerio_tx::ReactiveLimits,
    p: f64,
) -> Result<(f64, f64), String> {
    use powerio_tx::{CurveStyle, ReactiveLimits as R};
    let bounds = match limits {
        R::MinMax(l) => (l.minimum_reactive_power_mvar, l.maximum_reactive_power_mvar),
        R::CapabilityCurve(c) => {
            if c.points.len() < 2 || c.curve_style != CurveStyle::StraightLineYValues {
                return Err("Reactive capability needs at least two linear curve points".into());
            }
            let mut points = c.points.iter().collect::<Vec<_>>();
            points.sort_by(|a, b| a.active_power_mw.total_cmp(&b.active_power_mw));
            if points.iter().any(|v| {
                ![
                    v.active_power_mw,
                    v.minimum_reactive_power_mvar,
                    v.maximum_reactive_power_mvar,
                ]
                .iter()
                .all(|v| v.is_finite())
                    || v.minimum_reactive_power_mvar > v.maximum_reactive_power_mvar
            }) || points.windows(2).any(|w| {
                !w[0].active_power_mw.is_finite() || w[0].active_power_mw >= w[1].active_power_mw
            }) || p < points[0].active_power_mw
                || p > points.last().unwrap().active_power_mw
            {
                return Err("Active power is outside a valid reactive capability curve".into());
            }
            let pair = points
                .windows(2)
                .find(|w| p <= w[1].active_power_mw)
                .ok_or("Reactive curve interval missing")?;
            let t =
                (p - pair[0].active_power_mw) / (pair[1].active_power_mw - pair[0].active_power_mw);
            (
                (1.0 - t) * pair[0].minimum_reactive_power_mvar
                    + t * pair[1].minimum_reactive_power_mvar,
                (1.0 - t) * pair[0].maximum_reactive_power_mvar
                    + t * pair[1].maximum_reactive_power_mvar,
            )
        }
        _ => return Err("Unsupported reactive capability model".into()),
    };
    if !bounds.0.is_finite() || !bounds.1.is_finite() || bounds.0 > bounds.1 {
        return Err("Invalid reactive capability bounds".into());
    }
    Ok(bounds)
}

/// Solve detailed converter topology with reactive or AC-voltage controls. Reject
/// unsupported source controls and overlapping reduced links before modifying
/// the owned analysis copy. Source equipment and AC island topology stay intact.
pub fn solve_source(net: &BalancedNetwork, formulation: Problem) -> Result<AcDcSolution, String> {
    solve_source_with_regulators(net, formulation, false)
}
pub fn solve_source_with_regulators(
    net: &BalancedNetwork,
    formulation: Problem,
    automatic_regulators: bool,
) -> Result<AcDcSolution, String> {
    #[cfg(feature = "acopf")]
    if formulation == Problem::Acopf {
        if automatic_regulators {
            return Err("Joint OPF keeps discrete regulators fixed; automatic regulator loops are a power-flow study".into());
        }
        return crate::model::solve_joint_ac_dc(net);
    }

    if !matches!(formulation, Problem::AcPf | Problem::DcPf | Problem::DcOpf) {
        return Err("Joint AC/DC OPF requires the optional acopf backend; this build supports acpf, dcpf and dcopf".into());
    }
    if net.hvdc().iter().any(|h| h.in_service) {
        return Err("Detailed AC/DC solve refuses overlapping active reduced HVDC links; supply one authoritative detailed model".into());
    }
    let grid = dc_grid::from_network(net)?;
    let dc = dc_grid::solve(&grid)?;
    if dc.converters.is_empty() {
        return Err("No connected converters to couple".into());
    }
    let mut analysis = net.clone();
    let scale = if analysis.is_normalized() {
        analysis.base_mva()
    } else {
        1.0
    };
    let detailed = net
        .detailed_connectivity()
        .as_ref()
        .ok_or("No detailed source")?;
    let mut voltage_controls = Vec::new();
    let mut regulated_buses = std::collections::HashSet::new();
    let mut converter_reactive = Vec::new();
    let mut violations = dc.violations.clone();
    for c in &dc.converters {
        let q;
        let mut voltage = None;
        if let Some(source) = detailed
            .voltage_source_converters
            .iter()
            .find(|s| s.component.to_string() == c.id)
        {
            if source.maximum_modulation_index.is_some() || source.valve_u0_kv.is_some() {
                return Err(format!(
                    "{} requires a converter-specific AC/DC voltage relation",
                    c.id
                ));
            }
            match source.voltage_regulator_on {
                Some(true) => {
                    if formulation != Problem::AcPf {
                        return Err("Converter AC voltage control requires AC power flow or joint AC/DC OPF".into());
                    }
                    let target = voltage_target(net, source, c.ac_bus, &mut regulated_buses)?;
                    let bounds = reactive_bounds(
                        source
                            .reactive_limits
                            .as_ref()
                            .ok_or("Converter voltage control requires reactive limits")?,
                        c.ac_power_mw,
                    )?;
                    q = source
                        .reactive_power_setpoint_mvar
                        .unwrap_or(0.0)
                        .clamp(bounds.0, bounds.1);
                    voltage = Some((target, bounds));
                }
                Some(false) => {
                    q = source
                        .reactive_power_setpoint_mvar
                        .ok_or("Fixed-Q converter target missing")?;
                    if let Some(l) = &source.reactive_limits {
                        let (lo, hi) = reactive_bounds(l, c.ac_power_mw)?;
                        if q < lo || q > hi {
                            violations
                                .push(format!("{} fixed reactive target exceeds capability", c.id));
                        }
                    }
                }
                None => return Err("Converter AC control mode must be explicit".into()),
            }
        } else if let Some(source) = detailed
            .line_commutated_converters
            .iter()
            .find(|s| s.component.to_string() == c.id)
        {
            let pf = lcc_power_factor(source)?;
            q = c.ac_power_mw.abs() * (1.0 - pf * pf).sqrt() / pf;
            let rectifier = source.operating_mode
                == Some(powerio_tx::LineCommutatedConverterOperatingMode::Rectifier);
            if (rectifier && (c.ac_power_mw < -1e-7 || c.current_ka < -1e-8))
                || (!rectifier && (c.ac_power_mw > 1e-7 || c.current_ka > 1e-8))
            {
                return Err(format!(
                    "{} power direction conflicts with its LCC operating mode",
                    c.id
                ));
            }
        } else {
            return Err("Solved converter has no source record".into());
        }
        if !q.is_finite() {
            return Err("Converter reactive target is not finite".into());
        }
        if !net
            .buses()
            .iter()
            .any(|b| b.id.0 == c.ac_bus && b.kind != powerio::BusType::Isolated)
        {
            return Err("Converter AC bus unavailable".into());
        }
        let row = analysis.loads().len();
        let mut load = Load::new(BusId(c.ac_bus), c.ac_power_mw / scale, q / scale);
        load.uid = Some(format!("tellegen:dc-terminal:{}", c.id));
        if analysis.loads().iter().any(|l| l.uid == load.uid) {
            return Err("AC/DC analysis load identity collision".into());
        }
        analysis.loads_mut().push(load);
        if let Some((target, (lo, hi))) = voltage {
            voltage_controls.push(crate::regulation::Control {
                id: c.id.clone(),
                setting: crate::regulation::Setting::ReactiveLoad(row),
                measurement: crate::regulation::Measurement::Voltage(c.ac_bus),
                low: target - 1e-6,
                high: target + 1e-6,
                min: lo / scale,
                max: hi / scale,
                positions: vec![],
                units: if net.is_normalized() { "pu Q" } else { "Mvar" },
            });
        }
        converter_reactive.push((c.id.clone(), q));
    }
    // Only this qualified path removes the detailed records from the numerical
    // AC copy: every connected converter has already been solved and injected above.
    let d = std::sync::Arc::make_mut(
        analysis
            .detailed_connectivity_mut()
            .as_mut()
            .ok_or("No detailed source")?,
    );
    d.voltage_source_converters.clear();
    d.line_commutated_converters.clear();
    let mut controls = Vec::new();
    let ac = if !voltage_controls.is_empty() || automatic_regulators {
        if formulation != Problem::AcPf {
            return Err("Automatic source regulators require AC power flow".into());
        }
        if automatic_regulators {
            voltage_controls.extend(crate::regulation::source_controls(&analysis)?);
        }
        let regulated = crate::regulation::run(analysis, voltage_controls)?;
        for (id, q) in &mut converter_reactive {
            if let Some(c) = regulated.controls.iter().find(|c| &c.id == id) {
                *q = c.setting * scale;
            }
        }
        controls = regulated.controls;
        for c in &controls {
            if c.status != "regulated" {
                violations.push(format!("{}: {}", c.id, c.status));
            }
        }
        regulated.solution
    } else {
        crate::api::solve_network(
            &analysis,
            &SolveRequest {
                formulation,
                ..Default::default()
            },
        )?
    };
    let solved_buses: std::collections::HashSet<_> = ac
        .va
        .as_ref()
        .or(ac.lmp.as_ref())
        .ok_or("AC calculation has no bus coverage")?
        .iter()
        .map(|b| b.bus)
        .collect();
    if let Some(c) = dc
        .converters
        .iter()
        .find(|c| !solved_buses.contains(&c.ac_bus))
    {
        return Err(format!("{} is attached to an AC island omitted by the calculation; the DC transfer cannot be applied", c.id));
    }
    violations.extend(crate::regulation::operating_violations(net, &ac));
    Ok(AcDcSolution { dc, ac, violations, controls, converter_reactive, scope: "Steady-state AC/DC power flow with VSC reactive/voltage control and explicit fixed-power-factor LCC models. Source regulators run only when requested. DC OPF optimizes AC generation at fixed converter transfers. Commutation dynamics are not modeled." })
}

pub fn solve_module_json(module_json: &str, formulation: Problem) -> Result<String, String> {
    solve_module_json_with_regulators(module_json, formulation, false)
}
pub fn solve_module_json_with_regulators(
    module_json: &str,
    formulation: Problem,
    automatic_regulators: bool,
) -> Result<String, String> {
    let net = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?.into_value();
    serde_json::to_string(&solve_source_with_regulators(
        &net,
        formulation,
        automatic_regulators,
    )?)
    .map_err(|e| e.to_string())
}

#[cfg(all(test, feature = "sensitivity"))]
mod tests {
    use super::*;
    #[test]
    fn fixed_q_coupling_balances_and_preserves_source() {
        let net = dc_grid::tests::imported_fixture();
        let before = serde_json::to_string(&net).unwrap();
        for mode in [Problem::AcPf, Problem::DcPf, Problem::DcOpf] {
            let result = solve_source(&net, mode).unwrap();
            assert_eq!(result.dc.converters.len(), 3);
            if let Some(injections) = &result.ac.injections {
                let p2 = injections.iter().find(|b| b.bus == 2).unwrap().p;
                let demand2: f64 = net
                    .loads()
                    .iter()
                    .filter(|l| l.bus.0 == 2 && l.in_service)
                    .map(|l| l.p)
                    .sum();
                let converter = result.dc.converters.iter().find(|c| c.ac_bus == 2).unwrap();
                assert!((p2 + demand2 + converter.ac_power_mw).abs() < 1e-6);
            }
            assert!(result.ac.va.as_ref().unwrap().iter().any(|b| b.bus == 4));
            assert_eq!(serde_json::to_string(&net).unwrap(), before);
            let pac: f64 = result.dc.converters.iter().map(|c| c.ac_power_mw).sum();
            let losses: f64 = result.dc.branches.iter().map(|b| b.loss_mw).sum::<f64>()
                + result.dc.converters.iter().map(|c| c.loss_mw).sum::<f64>();
            assert!((pac - losses).abs() < 1e-7);
        }
    }
    #[test]
    fn detailed_converter_q_uses_withdrawal_convention() {
        for q in [-7.0, 13.0] {
            let mut n = dc_grid::tests::imported_fixture();
            let d = std::sync::Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap());
            let c = &mut d.voltage_source_converters[1];
            c.reactive_power_setpoint_mvar = Some(q);
            c.reactive_limits = Some(serde_json::from_value(serde_json::json!({"kind":"min_max","limits":{"minimum_reactive_power_mvar":-8.0,"maximum_reactive_power_mvar":14.0}})).unwrap());
            let r = solve_source(&n, Problem::AcPf).unwrap();
            let converter = r.dc.converters.iter().find(|c| c.ac_bus == 2).unwrap();
            assert_eq!(
                r.converter_reactive
                    .iter()
                    .find(|(id, _)| id == &converter.id)
                    .unwrap()
                    .1,
                q
            );
            let injection =
                r.ac.injections
                    .unwrap()
                    .into_iter()
                    .find(|b| b.bus == 2)
                    .unwrap();
            assert!((injection.q + q).abs() < 1e-6);
            assert!(!r.violations.iter().any(|v| v.contains("reactive target")));
        }
    }

    #[test]
    fn reactive_capability_interpolates_without_extrapolation() {
        let limits=serde_json::from_value(serde_json::json!({"kind":"capability_curve","limits":{"curve_style":"straight_line_y_values","points":[
            {"active_power_mw":-100.0,"minimum_reactive_power_mvar":-20.0,"maximum_reactive_power_mvar":40.0},
            {"active_power_mw":100.0,"minimum_reactive_power_mvar":-10.0,"maximum_reactive_power_mvar":20.0}
        ]}})).unwrap();
        assert_eq!(reactive_bounds(&limits, 0.0).unwrap(), (-15.0, 30.0));
        assert_eq!(reactive_bounds(&limits, 50.0).unwrap(), (-12.5, 25.0));
        assert_eq!(reactive_bounds(&limits, -50.0).unwrap(), (-17.5, 35.0));
        assert!(reactive_bounds(&limits, 101.0).is_err());
    }
    #[test]
    fn converter_voltage_control_and_q_limit() {
        let mut n = dc_grid::tests::imported_fixture();
        let d = std::sync::Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap());
        let c = &mut d.voltage_source_converters[1];
        c.voltage_regulator_on = Some(true);
        c.voltage_setpoint_kv = Some(138.0 * 1.02);
        c.reactive_limits=Some(serde_json::from_value(serde_json::json!({"kind":"min_max","limits":{"minimum_reactive_power_mvar":-200.0,"maximum_reactive_power_mvar":200.0}})).unwrap());
        let r = solve_source(&n, Problem::AcPf).unwrap();
        assert_eq!(r.controls[0].status, "regulated");
        assert!((r.controls[0].measured - 1.02).abs() < 3e-6);
        let c = &mut std::sync::Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters[1];
        c.reactive_limits=Some(serde_json::from_value(serde_json::json!({"kind":"min_max","limits":{"minimum_reactive_power_mvar":-0.1,"maximum_reactive_power_mvar":0.1}})).unwrap());
        let r = solve_source(&n, Problem::AcPf).unwrap();
        assert_eq!(r.controls[0].status, "at_limit");
        assert!(!r.violations.is_empty());
    }
    #[test]
    fn lcc_consumes_reactive_power_at_both_ends() {
        let mut module: serde_json::Value =
            serde_json::from_str(include_str!("../tests/data/transmission-mtdc.pio.json")).unwrap();
        let d = &mut module["value"]["data"]["detailed_connectivity"];
        let mut cs = d
            .as_object_mut()
            .unwrap()
            .remove("voltage_source_converters")
            .unwrap();
        for c in cs.as_array_mut().unwrap() {
            c["reactive_model"] = serde_json::json!("fixed_power_factor");
            c["operating_mode"] =
                serde_json::json!(
                    if c["target_active_power_mw"].as_f64().unwrap_or(0.0) < 0.0 {
                        "inverter"
                    } else {
                        "rectifier"
                    }
                );
            c["power_factor"] = serde_json::json!(0.95);
        }
        d["line_commutated_converters"] = cs;
        let input = module
            .to_string()
            .replace("voltage_source_converter", "line_commutated_converter");
        let n = crate::ir::balanced_module(crate::ir::deserialize_module(&input).unwrap())
            .unwrap()
            .into_value();
        let r = solve_source(&n, Problem::AcPf).unwrap();
        for ((id, q), c) in r.converter_reactive.iter().zip(&r.dc.converters) {
            assert_eq!(id, &c.id);
            assert!(*q > 0.0);
            assert!(
                (*q - c.ac_power_mw.abs() * (1.0_f64 - 0.95 * 0.95).sqrt() / 0.95).abs() < 1e-8
            );
        }
    }
    #[test]
    fn ac_control_and_duplicate_equivalent_are_not_ignored() {
        let mut net = dc_grid::tests::imported_fixture();
        std::sync::Arc::make_mut(net.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters[0]
            .voltage_regulator_on = Some(true);
        assert!(solve_source(&net, Problem::AcPf)
            .unwrap_err()
            .contains("voltage control"));
        net.hvdc_mut().push(powerio::Hvdc::new(BusId(1), BusId(2)));
        assert!(solve_source(&net, Problem::AcPf)
            .unwrap_err()
            .contains("overlapping"));
    }
}
