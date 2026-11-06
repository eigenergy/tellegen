//! Qualified steady-state AC/DC studies with separable, fixed converter controls.
//!
//! For the stated `Pac = Udc Idc + loss(Idc)` model, DC controls do not depend
//! on AC voltage. Solve DC first, then apply each terminal's Pac and fixed Q
//! exactly once as an AC load. This is an exact block solve of that restricted
//! model, not a unified converter model or joint AC/DC economic optimization.
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
    pub scope: &'static str,
}

/// Solve detailed source VSC topology with fixed reactive-power controls. Reject
/// unsupported source controls and overlapping reduced links before modifying
/// the owned analysis copy. Source equipment and AC island topology stay intact.
pub fn solve_source(net: &BalancedNetwork, formulation: Problem) -> Result<AcDcSolution, String> {
    if !matches!(formulation, Problem::AcPf | Problem::DcPf | Problem::DcOpf) {
        return Err("Detailed AC/DC studies currently support acpf, dcpf or dcopf with fixed converter controls; joint AC/DC OPF is unavailable".into());
    }
    if net.hvdc().iter().any(|h| h.in_service) {
        return Err("Detailed AC/DC solve refuses overlapping active reduced HVDC links; supply one authoritative detailed model".into());
    }
    let grid = dc_grid::from_network(net)?;
    let dc = dc_grid::solve(&grid)?;
    if dc.converters.is_empty() {
        return Err("No connected VSC converters to couple".into());
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
    for c in &dc.converters {
        let source = detailed
            .voltage_source_converters
            .iter()
            .find(|s| s.component.to_string() == c.id)
            .ok_or("Solved converter has no source record")?;
        if source.voltage_regulator_on != Some(false) {
            return Err(format!("{} needs an explicitly disabled AC voltage regulator; AC voltage control is not implemented in this fixed-Q study", c.id));
        }
        if source.maximum_modulation_index.is_some() || source.valve_u0_kv.is_some() {
            return Err(format!(
                "{} requires a converter-specific AC/DC voltage relation",
                c.id
            ));
        }
        let q = source
            .reactive_power_setpoint_mvar
            .ok_or_else(|| format!("{} has no fixed reactive-power target", c.id))?;
        if !q.is_finite() {
            return Err("Converter reactive target is not finite".into());
        }
        if source.reactive_limits.is_some() {
            return Err(format!(
                "{} has reactive capability limits requiring a qualified capability model",
                c.id
            ));
        }
        if !net
            .buses()
            .iter()
            .any(|b| b.id.0 == c.ac_bus && b.kind != powerio::BusType::Isolated)
        {
            return Err(format!("{} has an unavailable AC bus", c.id));
        }
        let mut load = Load::new(BusId(c.ac_bus), c.ac_power_mw / scale, q / scale);
        load.uid = Some(format!("tellegen:dc-terminal:{}", c.id));
        if analysis.loads().iter().any(|l| l.uid == load.uid) {
            return Err("AC/DC analysis load identity collision".into());
        }
        analysis.loads_mut().push(load);
    }
    // Only this qualified path removes the detailed records from the numerical
    // AC copy: every connected VSC has already been solved and injected above.
    let d = std::sync::Arc::make_mut(
        analysis
            .detailed_connectivity_mut()
            .as_mut()
            .ok_or("No detailed source")?,
    );
    d.voltage_source_converters.clear();
    d.line_commutated_converters.clear();
    let ac = crate::api::solve_network(
        &analysis,
        &SolveRequest {
            formulation,
            ..Default::default()
        },
    )?;
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
    let mut violations = dc.violations.clone();
    if let Some(vm) = &ac.vm {
        for v in vm {
            if let Some(b) = net.buses().iter().find(|b| b.id.0 == v.bus) {
                if v.value < b.vmin - 1e-7 || v.value > b.vmax + 1e-7 {
                    violations.push(format!("AC bus {} exceeds its voltage range", v.bus));
                }
            }
        }
    }
    Ok(AcDcSolution { dc, ac, violations, scope: "Fixed-control VSC DC grid coupled to AC terminal injections. DC PF/OPF use an AC-network DC approximation; OPF optimizes AC generation only. Converter AC-voltage control, joint AC/DC optimization and dynamic behavior are not modeled." })
}

pub fn solve_module_json(module_json: &str, formulation: Problem) -> Result<String, String> {
    let net = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?.into_value();
    serde_json::to_string(&solve_source(&net, formulation)?).map_err(|e| e.to_string())
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
    fn ac_control_and_duplicate_equivalent_are_not_ignored() {
        let mut net = dc_grid::tests::imported_fixture();
        std::sync::Arc::make_mut(net.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters[0]
            .voltage_regulator_on = Some(true);
        assert!(solve_source(&net, Problem::AcPf)
            .unwrap_err()
            .contains("voltage regulator"));
        net.hvdc_mut().push(powerio::Hvdc::new(BusId(1), BusId(2)));
        assert!(solve_source(&net, Problem::AcPf)
            .unwrap_err()
            .contains("overlapping"));
    }
}
