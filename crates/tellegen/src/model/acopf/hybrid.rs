//! Joint polar AC / resistive DC OPF. Converter Pac is an expression, not a fixed
//! injection. Discrete taps and shunt sections remain at their source settings.
use super::*;
use crate::{
    ac_dc::{self, AcDcSolution},
    dc_grid::{self, DcControl},
};
use powerio::BalancedNetwork;
use powerio_tx::ReactiveLimits;

fn column(p: &mut NlProblemParts, name: String, lo: f64, hi: f64, start: f64) -> usize {
    let i = p.x0.len();
    p.x0.push(start.clamp(lo, hi));
    p.x_l.push(lo);
    p.x_u.push(hi);
    p.var_names.push(name);
    i
}
fn row(p: &mut NlProblemParts, name: String, e: Expr, lo: f64, hi: f64) {
    p.constraints.push(e);
    p.g_l.push(lo);
    p.g_u.push(hi);
    p.con_names.push(name);
}
fn bounds(lo: Option<f64>, hi: Option<f64>, name: &str) -> Result<(f64, f64), String> {
    let (lo, hi) = (
        lo.ok_or_else(|| format!("Joint OPF requires {name} lower limit"))?,
        hi.ok_or_else(|| format!("Joint OPF requires {name} upper limit"))?,
    );
    if !lo.is_finite() || !hi.is_finite() || lo > hi {
        return Err(format!("Invalid {name} bounds"));
    }
    Ok((lo, hi))
}

pub(crate) fn solve_joint_ac_dc(net: &BalancedNetwork) -> Result<AcDcSolution, String> {
    if net.hvdc().iter().any(|h| h.in_service) {
        return Err("Joint AC/DC OPF refuses overlapping reduced HVDC links".into());
    }
    let grid = dc_grid::from_network(net)?;
    let start = dc_grid::solve(&grid)?;
    let d = net
        .detailed_connectivity()
        .as_ref()
        .ok_or("Detailed converters missing")?;
    let mut analysis = net.clone();
    let ad = Arc::make_mut(analysis.detailed_connectivity_mut().as_mut().unwrap());
    ad.voltage_source_converters.clear();
    ad.line_commutated_converters.clear();
    let instance = AcOpfInstance::from_network(analysis).map_err(|e| e.to_string())?;
    let converters = grid
        .converters
        .iter()
        .filter(|c| c.in_service)
        .collect::<Vec<_>>();
    let mut voltage_columns = Vec::new();
    let mut current_columns = Vec::new();
    let mut q_columns = Vec::new();
    let mut ac_rows = Vec::new();
    let mut regulated_buses = std::collections::HashSet::new();
    let mut model = compile_ac_opf_model_with(&instance, |prep, cols, rows, p| {
        for node in &grid.nodes {
            let (lo, hi) = node.reference_kv.map_or((-INF, INF), |v| (v, v));
            let v = start
                .nodes
                .iter()
                .find(|v| v.id == node.id)
                .unwrap()
                .voltage_kv;
            voltage_columns.push(column(p, format!("dc_voltage[{}]", node.id), lo, hi, v));
        }
        let node_index = |id: &str| grid.nodes.iter().position(|n| n.id == id).unwrap();
        let mut kcl = vec![Vec::new(); grid.nodes.len()];
        for b in grid.branches.iter().filter(|b| b.in_service) {
            let (f, t) = (node_index(&b.from), node_index(&b.to));
            let current = scaled(
                1.0 / b.resistance_ohm,
                subtract(variable(voltage_columns[f]), variable(voltage_columns[t])),
            );
            kcl[f].push(current.clone());
            kcl[t].push(scaled(-1.0, current.clone()));
            if let Some(limit) = b.current_limit_ka {
                row(
                    p,
                    format!("dc_branch_current[{}]", b.id),
                    current,
                    -limit,
                    limit,
                );
            }
        }
        for c in &converters {
            let (pmin, pmax) = bounds(
                c.active_min_mw,
                c.active_max_mw,
                &format!("{} active power", c.id),
            )?;
            let (vmin, vmax) = bounds(
                c.voltage_min_kv,
                c.voltage_max_kv,
                &format!("{} DC voltage", c.id),
            )?;
            if vmin <= 0.0 {
                return Err("Joint OPF requires positive minimum converter DC voltage".into());
            }
            let (f, t) = (node_index(&c.positive), node_index(&c.negative));
            let initial = start.converters.iter().find(|s| s.id == c.id).unwrap();
            let imax = c.current_limit_ka.unwrap_or(INF);
            let i = column(
                p,
                format!("dc_current[{}]", c.id),
                if c.current_direction == Some(dc_grid::DcCurrentDirection::Positive) {
                    0.0
                } else {
                    -imax
                },
                if c.current_direction == Some(dc_grid::DcCurrentDirection::Negative) {
                    0.0
                } else {
                    imax
                },
                initial.current_ka,
            );
            current_columns.push(i);
            let u = subtract(variable(voltage_columns[f]), variable(voltage_columns[t]));
            let loss = sum(vec![
                constant(c.loss[0]),
                scaled(c.loss[1], unary(UnaryOp::Abs, variable(i))),
                scaled(c.loss[2], square(variable(i))),
            ]);
            let pac = add(multiply(u.clone(), variable(i)), loss);
            row(
                p,
                format!("converter_power[{}]", c.id),
                pac.clone(),
                pmin,
                pmax,
            );
            row(
                p,
                format!("converter_voltage[{}]", c.id),
                u.clone(),
                vmin,
                vmax,
            );
            match c.control {
                DcControl::Voltage { kv } => {
                    row(p, format!("dc_voltage_schedule[{}]", c.id), u, kv, kv)
                }
                DcControl::ActivePower { .. } => {} // Dispatch variable in joint OPF.
                DcControl::Current { ka } => row(
                    p,
                    format!("dc_current_schedule[{}]", c.id),
                    variable(i),
                    ka,
                    ka,
                ),
                DcControl::Droop { .. } => {
                    return Err("Joint OPF source droop dispatch is not qualified".into())
                }
            }
            kcl[f].push(scaled(-1.0, variable(i)));
            kcl[t].push(variable(i));
            let bus = prep
                .bus_ids
                .iter()
                .position(|b| b.0 == c.ac_bus)
                .ok_or("Converter AC bus omitted by OPF")?;
            ac_rows.push(bus);
            let q = if let Some(vsc) = d
                .voltage_source_converters
                .iter()
                .find(|v| v.component.to_string() == c.id)
            {
                if vsc.maximum_modulation_index.is_some() || vsc.valve_u0_kv.is_some() {
                    return Err("Joint OPF cannot ignore a converter AC/DC voltage relation".into());
                }
                let limit=match &vsc.reactive_limits {
                    Some(ReactiveLimits::MinMax(l))=>bounds(Some(l.minimum_reactive_power_mvar),Some(l.maximum_reactive_power_mvar),"converter reactive power")?,
                    _=>return Err("Joint OPF requires explicit min/max converter reactive capability; P-dependent curves are not yet qualified".into()),
                };
                match vsc.voltage_regulator_on {
                    Some(true) => {
                        let target =
                            ac_dc::voltage_target(net, vsc, c.ac_bus, &mut regulated_buses)?;
                        row(
                            p,
                            format!("ac_voltage_schedule[{}]", c.id),
                            variable(cols.vm[bus]),
                            target,
                            target,
                        );
                        column(
                            p,
                            format!("converter_q[{}]", c.id),
                            limit.0,
                            limit.1,
                            vsc.reactive_power_setpoint_mvar.unwrap_or(0.0),
                        )
                    }
                    Some(false) => {
                        let q = vsc
                            .reactive_power_setpoint_mvar
                            .ok_or("Fixed-Q converter target missing")?;
                        if !q.is_finite() || q < limit.0 || q > limit.1 {
                            return Err("Fixed-Q converter target violates capability".into());
                        }
                        column(p, format!("converter_q[{}]", c.id), q, q, q)
                    }
                    None => return Err("Converter AC control mode must be explicit".into()),
                }
            } else {
                let lcc = d
                    .line_commutated_converters
                    .iter()
                    .find(|v| v.component.to_string() == c.id)
                    .ok_or("LCC source missing")?;
                let pf = ac_dc::lcc_power_factor(lcc)?;
                let rectifier = lcc.operating_mode
                    == Some(powerio_tx::LineCommutatedConverterOperatingMode::Rectifier);
                let slope = (if rectifier { 1.0 } else { -1.0 }) * (1.0 - pf * pf).sqrt() / pf;
                let q = column(
                    p,
                    format!("converter_q[{}]", c.id),
                    0.0,
                    INF,
                    initial.ac_power_mw * slope,
                );
                row(
                    p,
                    format!("lcc_power_factor[{}]", c.id),
                    subtract(variable(q), scaled(slope, pac.clone())),
                    0.0,
                    0.0,
                );
                q
            };
            q_columns.push(q);
            p.constraints[rows.p_balance[bus]] = subtract(
                p.constraints[rows.p_balance[bus]].clone(),
                scaled(1.0 / prep.base_mva, pac),
            );
            p.constraints[rows.q_balance[bus]] = subtract(
                p.constraints[rows.q_balance[bus]].clone(),
                scaled(1.0 / prep.base_mva, variable(q)),
            );
        }
        for (i, terms) in kcl.into_iter().enumerate() {
            if grid.nodes[i].reference_kv.is_none() {
                row(
                    p,
                    format!("dc_kcl[{}]", grid.nodes[i].id),
                    sum(terms),
                    0.0,
                    0.0,
                );
            }
        }
        Ok(())
    })?;
    #[cfg(test)]
    {
        let errors = super::tests::directional_derivative_errors(&model, &mut model.tnlp()?);
        assert!(
            errors.jacobian_scaled < 2e-6 && errors.hessian_scaled < 2e-6,
            "{errors:?}"
        );
    }
    // A hybrid fingerprint must include the DC model and AC converter controls.
    let mut hash = Sha256::new();
    hash.update(&model.fingerprint);
    hash.update(serde_json::to_vec(&grid).map_err(|e| e.to_string())?);
    hash.update(serde_json::to_vec(d).map_err(|e| e.to_string())?);
    model.fingerprint = hash
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    let mut dc_result = None;
    let mut reactive = Vec::new();
    let accepted = solve_compiled(model, None, POUNCE_OPTIONS, |model, x| {
        let mut check_grid = grid.clone();
        for (j, c) in converters.iter().enumerate() {
            let f = grid.nodes.iter().position(|n| n.id == c.positive).unwrap();
            let t = grid.nodes.iter().position(|n| n.id == c.negative).unwrap();
            let u = x[voltage_columns[f]] - x[voltage_columns[t]];
            let i = x[current_columns[j]];
            if c.loss[1] > 0.0 && i.abs() < 1e-9 {
                return Err("Joint OPF reached the absolute-current loss kink; smooth KKT optimality is not qualified".into());
            }
            let pac = u * i + c.loss[0] + c.loss[1] * i.abs() + c.loss[2] * i * i;
            if matches!(c.control, DcControl::ActivePower { .. }) {
                check_grid
                    .converters
                    .iter_mut()
                    .find(|v| v.id == c.id)
                    .unwrap()
                    .control = DcControl::ActivePower { mw: pac };
            }
            model.preparation.buses.p_d[ac_rows[j]] += pac / model.preparation.base_mva;
            model.preparation.buses.q_d[ac_rows[j]] += x[q_columns[j]] / model.preparation.base_mva;
            let q = x[q_columns[j]];
            if let Some(vsc) = d
                .voltage_source_converters
                .iter()
                .find(|v| v.component.to_string() == c.id)
            {
                if vsc.voltage_regulator_on == Some(true) {
                    let bus = net.buses().iter().find(|b| b.id.0 == c.ac_bus).unwrap();
                    let target = vsc.voltage_setpoint_kv.unwrap() / bus.base_kv;
                    if (x[model.columns.vm[ac_rows[j]]] - target).abs() > 1e-6 {
                        return Err("Independent converter AC voltage check failed".into());
                    }
                }
            } else {
                let lcc = d
                    .line_commutated_converters
                    .iter()
                    .find(|v| v.component.to_string() == c.id)
                    .unwrap();
                let pf = ac_dc::lcc_power_factor(lcc)?;
                if (q - pac.abs() * (1.0 - pf * pf).sqrt() / pf).abs() > 1e-6 {
                    return Err("Independent LCC reactive power check failed".into());
                }
            }
            reactive.push((c.id.clone(), q));
        }
        for (j, n) in check_grid.nodes.iter_mut().enumerate() {
            n.initial_kv = x[voltage_columns[j]];
        }
        let checked = dc_grid::solve(&check_grid)?;
        if !checked.violations.is_empty() {
            return Err(format!(
                "Joint OPF violates DC limits: {}",
                checked.violations.join(", ")
            ));
        }
        for (j, c) in converters.iter().enumerate() {
            let s = checked.converters.iter().find(|s| s.id == c.id).unwrap();
            if (s.current_ka - x[current_columns[j]]).abs() > 1e-6 {
                return Err("Independent DC check disagrees with joint OPF".into());
            }
        }
        for (j, n) in checked.nodes.iter().enumerate() {
            if (n.voltage_kv - x[voltage_columns[j]]).abs() > 1e-5 {
                return Err("Independent DC voltage check disagrees with joint OPF".into());
            }
        }
        dc_result = Some(checked);
        Ok(())
    })?;
    let ac = crate::api::ac_opf_assemble(net, &accepted)?;
    Ok(AcDcSolution{dc:dc_result.ok_or("Missing joint DC result")?,ac,controls:vec![],converter_reactive:reactive,violations:vec![],scope:"Joint nonlinear AC/DC OPF: AC generation and converter active transfers optimized together, with resistive DC losses, fixed-Q or scheduled AC-voltage VSCs and fixed-power-factor LCCs. Local optimum only. Transformer taps and shunt sections stay fixed. Missing equipment ratings remain unchecked; converter voltage schedules are hard constraints."})
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn bounded_fixture() -> BalancedNetwork {
        let mut n = dc_grid::tests::imported_fixture();
        for c in &mut Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters
        {
            c.minimum_active_power_mw = Some(-150.0);
            c.maximum_active_power_mw = Some(150.0);
            c.minimum_dc_voltage_kv = Some(280.0);
            c.maximum_dc_voltage_kv = Some(360.0);
            c.reactive_limits=Some(serde_json::from_value(serde_json::json!({"kind":"min_max","limits":{"minimum_reactive_power_mvar":-200.0,"maximum_reactive_power_mvar":200.0}})).unwrap());
        }
        n
    }
    #[test]
    fn joint_dispatch_balances_and_changes_transfers() {
        let n = bounded_fixture();
        let before = serde_json::to_string(&n).unwrap();
        let fixed = dc_grid::solve(&dc_grid::from_network(&n).unwrap()).unwrap();
        let joint = solve_joint_ac_dc(&n).unwrap();
        let mut baseline = n.clone();
        *baseline.detailed_connectivity_mut() = None;
        for c in &fixed.converters {
            baseline.loads_mut().push(powerio::Load::new(
                powerio::BusId(c.ac_bus),
                c.ac_power_mw,
                0.0,
            ));
        }
        let fixed_ac = crate::api::solve_network(
            &baseline,
            &crate::api::SolveRequest {
                formulation: crate::Problem::Acopf,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(joint.ac.objective.unwrap() < fixed_ac.objective.unwrap() - 1e-3);
        assert!(joint.violations.is_empty());
        assert!(joint.dc.residual < 1e-8);
        assert!(joint
            .dc
            .converters
            .iter()
            .zip(&fixed.converters)
            .any(|(a, b)| (a.ac_power_mw - b.ac_power_mw).abs() > 1.0));
        let pac: f64 = joint.dc.converters.iter().map(|c| c.ac_power_mw).sum();
        let loss: f64 = joint
            .dc
            .converters
            .iter()
            .map(|c| c.loss_mw)
            .chain(joint.dc.branches.iter().map(|b| b.loss_mw))
            .sum();
        assert!((pac - loss).abs() < 1e-6);
        assert_eq!(serde_json::to_string(&n).unwrap(), before);
    }
    #[test]
    fn binding_150_mw_converter_limit_is_accepted() {
        let mut n = bounded_fixture();
        n.loads_mut()[0].p = 300.0;
        for (i, g) in n.generators_mut().iter_mut().enumerate() {
            g.pmax = 1000.0;
            g.cost.as_mut().unwrap().coeffs = vec![0.0, if i == 0 { 1.0 } else { 100.0 }, 0.0];
        }
        for b in &mut n.branches_mut()[1..] {
            b.rate_a = 1000.0;
        }
        let result = solve_joint_ac_dc(&n).unwrap();
        assert!(
            result
                .dc
                .converters
                .iter()
                .any(|c| (c.ac_power_mw.abs() - 150.0).abs() < 1e-4),
            "{:?}",
            result.dc.converters
        );
        assert!(result.violations.is_empty());
    }

    #[test]
    fn missing_dispatch_limits_are_rejected() {
        assert!(solve_joint_ac_dc(&dc_grid::tests::imported_fixture())
            .unwrap_err()
            .contains("requires"));
    }
    #[test]
    fn joint_voltage_schedule_and_lcc_models() {
        let mut n = bounded_fixture();
        let c = &mut Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters[1];
        c.voltage_regulator_on = Some(true);
        c.voltage_setpoint_kv = Some(138.0 * 1.02);
        let r = solve_joint_ac_dc(&n).unwrap();
        assert!(
            (r.ac
                .vm
                .unwrap()
                .into_iter()
                .find(|v| v.bus == 2)
                .unwrap()
                .value
                - 1.02)
                .abs()
                < 1e-6
        );
        let mut n = bounded_fixture();
        let d = Arc::make_mut(n.detailed_connectivity_mut().as_mut().unwrap());
        for mut c in std::mem::take(&mut d.voltage_source_converters) {
            let inverter = c.target_active_power_mw.unwrap_or(0.0) < 0.0;
            if inverter {
                c.maximum_active_power_mw = Some(-1.0);
            } else {
                c.minimum_active_power_mw = Some(1.0);
            }
            let mut v = serde_json::to_value(c).unwrap();
            v["reactive_model"] = serde_json::json!("fixed_power_factor");
            v["operating_mode"] =
                serde_json::json!(if inverter { "inverter" } else { "rectifier" });
            v["power_factor"] = serde_json::json!(0.95);
            d.line_commutated_converters
                .push(serde_json::from_value(v).unwrap());
        }
        let r = solve_joint_ac_dc(&n).unwrap();
        assert!(r.converter_reactive.iter().all(|(_, q)| *q > 0.0));
    }
}
