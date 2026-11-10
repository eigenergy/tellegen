//! Steady-state resistive DC grids with explicit two-terminal converter ports.
//!
//! Voltages are kV, currents kA and powers MW. Positive converter current is
//! injected into its positive terminal and withdrawn from its negative terminal.
//! Converter AC power is positive consumption: `Pac = Udc * Idc + loss(Idc)`.
//! This module does not approximate a DC grid with AC-network DC power flow.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcGrid {
    pub nodes: Vec<DcNode>,
    pub branches: Vec<DcBranch>,
    pub converters: Vec<DcConverter>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcNode {
    pub id: String,
    pub initial_kv: f64,
    /// A potential reference, not necessarily a physical earth electrode.
    pub reference_kv: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcBranch {
    pub id: String,
    pub from: String,
    pub to: String,
    pub resistance_ohm: f64,
    pub in_service: bool,
    pub current_limit_ka: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcControl {
    Voltage {
        kv: f64,
    },
    ActivePower {
        mw: f64,
    },
    Current {
        ka: f64,
    },
    /// Explicit Pac(U) slope in MW/kV. A positive slope reduces AC input when
    /// DC voltage rises. Format-specific droop constants require conversion.
    Droop {
        mw: f64,
        kv: f64,
        mw_per_kv: f64,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DcCurrentDirection {
    Positive,
    Negative,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcConverter {
    pub id: String,
    pub positive: String,
    pub negative: String,
    pub ac_bus: usize,
    pub in_service: bool,
    pub control: DcControl,
    /// An explicit unidirectional converter constraint, independent of AC loss.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_direction: Option<DcCurrentDirection>,
    /// `a + b * |I| + c * I²`, with I in kA and loss in MW.
    pub loss: [f64; 3],
    pub current_limit_ka: Option<f64>,
    pub active_min_mw: Option<f64>,
    pub active_max_mw: Option<f64>,
    pub voltage_min_kv: Option<f64>,
    pub voltage_max_kv: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DcGridSolution {
    pub iterations: usize,
    pub residual: f64,
    pub limits_checked: usize,
    pub limits_unspecified: usize,
    pub nodes: Vec<DcNodeResult>,
    pub branches: Vec<DcBranchResult>,
    pub converters: Vec<DcConverterResult>,
    /// A converged operating point can violate ratings. Never call it feasible
    /// solely because the nonlinear equations converged.
    pub violations: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DcNodeResult {
    pub id: String,
    pub voltage_kv: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DcBranchResult {
    pub id: String,
    pub current_ka: f64,
    pub loss_mw: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DcConverterResult {
    pub id: String,
    pub ac_bus: usize,
    pub voltage_kv: f64,
    pub current_ka: f64,
    pub ac_power_mw: f64,
    pub loss_mw: f64,
}

fn loss(c: &DcConverter, i: f64) -> (f64, f64) {
    (
        c.loss[0] + c.loss[1] * i.abs() + c.loss[2] * i * i,
        c.loss[1] * i.signum() + 2.0 * c.loss[2] * i,
    )
}
fn finite(value: f64, label: &str) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("DC grid {label} is not finite"))
    }
}
fn positive(value: f64, label: &str) -> Result<(), String> {
    finite(value, label)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(format!("DC grid {label} must be positive"))
    }
}

struct Prepared<'a> {
    grid: &'a DcGrid,
    branches: Vec<(usize, usize, &'a DcBranch)>,
    converters: Vec<(usize, usize, &'a DcConverter)>,
}
impl<'a> Prepared<'a> {
    fn new(grid: &'a DcGrid) -> Result<Self, String> {
        if grid.nodes.is_empty() {
            return Err("DC grid has no nodes".into());
        }
        // Dense Newton is intended for converter grids, not arbitrary enormous
        // imported meshes. Bound allocation before accepting untrusted JSON.
        if grid.nodes.len() + grid.converters.len() > 512 {
            return Err("DC grid exceeds the 512-variable dense solver limit".into());
        }
        let mut ids = HashSet::new();
        let mut rows = HashMap::new();
        for (row, n) in grid.nodes.iter().enumerate() {
            if n.id.is_empty() || !ids.insert(&n.id) {
                return Err("DC node identities must be nonempty and unique".into());
            }
            rows.insert(n.id.as_str(), row);
            finite(n.initial_kv, "initial voltage")?;
            if let Some(v) = n.reference_kv {
                finite(v, "reference voltage")?;
            }
        }
        let ends = |a: &str, b: &str| -> Result<(usize, usize), String> {
            let a = *rows
                .get(a)
                .ok_or_else(|| format!("Unknown DC terminal {a}"))?;
            let b = *rows
                .get(b)
                .ok_or_else(|| format!("Unknown DC terminal {b}"))?;
            if a == b {
                return Err("DC device has identical terminals".into());
            }
            Ok((a, b))
        };
        let mut branches = Vec::new();
        let mut equipment_ids = HashSet::new();
        for b in &grid.branches {
            if b.id.is_empty() || !equipment_ids.insert(&b.id) {
                return Err("DC equipment identities must be nonempty and unique".into());
            }
            let (f, t) = ends(&b.from, &b.to)?;
            positive(
                b.resistance_ohm,
                "line resistance (reduce ideal switches before solving)",
            )?;
            if let Some(limit) = b.current_limit_ka {
                positive(limit, "line current limit")?;
            }
            if b.in_service {
                branches.push((f, t, b));
            }
        }
        let mut converters = Vec::new();
        for c in &grid.converters {
            if c.id.is_empty() || !equipment_ids.insert(&c.id) {
                return Err("DC equipment identities must be nonempty and unique".into());
            }
            let (f, t) = ends(&c.positive, &c.negative)?;
            for k in c.loss {
                finite(k, "converter loss coefficient")?;
                if k < 0.0 {
                    return Err("DC converter loss coefficients must be nonnegative".into());
                }
            }
            for v in [c.current_limit_ka, c.voltage_min_kv, c.voltage_max_kv]
                .into_iter()
                .flatten()
            {
                positive(v, "converter limit")?;
            }
            for v in [c.active_min_mw, c.active_max_mw].into_iter().flatten() {
                finite(v, "active power limit")?;
            }
            if c.active_min_mw
                .zip(c.active_max_mw)
                .is_some_and(|(a, b)| a > b)
                || c.voltage_min_kv
                    .zip(c.voltage_max_kv)
                    .is_some_and(|(a, b)| a > b)
            {
                return Err("DC converter has reversed limits".into());
            }
            match c.control {
                DcControl::Voltage { kv } => positive(kv, "voltage target")?,
                DcControl::ActivePower { mw } => finite(mw, "power target")?,
                DcControl::Current { ka } => finite(ka, "current target")?,
                DcControl::Droop { mw, kv, mw_per_kv } => {
                    finite(mw, "droop power target")?;
                    positive(kv, "droop voltage target")?;
                    positive(mw_per_kv, "droop slope")?;
                }
            }
            if c.in_service {
                converters.push((f, t, c));
            }
        }
        // Include converter ports in connectivity, but never confuse this with
        // AC synchronous-island connectivity.
        let mut adjacent = vec![Vec::new(); grid.nodes.len()];
        for (a, b) in branches
            .iter()
            .map(|(a, b, _)| (*a, *b))
            .chain(converters.iter().map(|(a, b, _)| (*a, *b)))
        {
            adjacent[a].push(b);
            adjacent[b].push(a);
        }
        let mut seen = vec![false; grid.nodes.len()];
        for root in 0..seen.len() {
            if seen[root] {
                continue;
            }
            let mut stack = vec![root];
            let mut component = Vec::new();
            seen[root] = true;
            while let Some(a) = stack.pop() {
                component.push(a);
                for &b in &adjacent[a] {
                    if !seen[b] {
                        seen[b] = true;
                        stack.push(b);
                    }
                }
            }
            if !component
                .iter()
                .any(|&a| grid.nodes[a].reference_kv.is_some())
            {
                return Err(format!(
                    "DC island at {} has no potential reference",
                    grid.nodes[root].id
                ));
            }
            let references: Vec<_> = component
                .iter()
                .filter_map(|&a| grid.nodes[a].reference_kv)
                .collect();
            if references.iter().any(|v| (v - references[0]).abs() > 1e-9) {
                return Err("Different prescribed potentials in one DC island would introduce an unmodeled power source; use explicit converter voltage controls".into());
            }
            let controls: Vec<_> = converters
                .iter()
                .filter(|(a, _, _)| component.contains(a))
                .collect();
            if !controls.is_empty()
                && !controls.iter().any(|(_, _, c)| {
                    matches!(
                        c.control,
                        DcControl::Voltage { .. } | DcControl::Droop { .. }
                    )
                })
            {
                return Err(format!(
                    "DC island at {} has no voltage-regulating converter",
                    grid.nodes[root].id
                ));
            }
        }
        Ok(Self {
            grid,
            branches,
            converters,
        })
    }

    /// KCL in kA, converter power equations scaled by their initial voltage.
    /// Return the exact Jacobian for Newton and implicit sensitivities.
    fn equations(&self, x: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
        let n = self.grid.nodes.len();
        let size = n + self.converters.len();
        let mut f = vec![0.0; size];
        let mut j = vec![vec![0.0; size]; size];
        for &(a, b, line) in &self.branches {
            let g = 1.0 / line.resistance_ohm;
            let i = (x[a] - x[b]) * g;
            f[a] += i;
            f[b] -= i;
            j[a][a] += g;
            j[a][b] -= g;
            j[b][a] -= g;
            j[b][b] += g;
        }
        for (k, &(a, b, c)) in self.converters.iter().enumerate() {
            let row = n + k;
            let u = x[a] - x[b];
            let i = x[row];
            let (loss, dloss) = loss(c, i);
            f[a] -= i;
            f[b] += i;
            j[a][row] -= 1.0;
            j[b][row] += 1.0;
            let scale = (self.grid.nodes[a].initial_kv - self.grid.nodes[b].initial_kv)
                .abs()
                .max(1.0);
            let (du, di) = match c.control {
                DcControl::Voltage { kv } => {
                    f[row] = (u - kv) / scale;
                    (1.0 / scale, 0.0)
                }
                DcControl::Current { ka } => {
                    f[row] = i - ka;
                    (0.0, 1.0)
                }
                DcControl::ActivePower { mw } => {
                    f[row] = (u * i + loss - mw) / scale;
                    (i / scale, (u + dloss) / scale)
                }
                DcControl::Droop { mw, kv, mw_per_kv } => {
                    f[row] = (u * i + loss - mw + mw_per_kv * (u - kv)) / scale;
                    ((i + mw_per_kv) / scale, (u + dloss) / scale)
                }
            };
            j[row][a] = du;
            j[row][b] = -du;
            j[row][row] = di;
        }
        for (a, node) in self.grid.nodes.iter().enumerate() {
            if let Some(v) = node.reference_kv {
                f[a] = x[a] - v;
                j[a].fill(0.0);
                j[a][a] = 1.0;
            }
        }
        (f, j)
    }
}

/// Pivoted dense elimination. A singular Jacobian indicates missing/conflicting
/// controls or an ill-conditioned operating point; do not regularize it away.
fn linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, String> {
    let n = b.len();
    for k in 0..n {
        let pivot = (k..n)
            .max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs()))
            .unwrap();
        if a[pivot][k].abs() < 1e-12 {
            return Err(
                "DC grid Jacobian is singular: check references, islands and converter controls"
                    .into(),
            );
        }
        a.swap(k, pivot);
        b.swap(k, pivot);
        for i in k + 1..n {
            let m = a[i][k] / a[k][k];
            let (before, after) = a.split_at_mut(i);
            for (entry, pivot) in after[0][k..].iter_mut().zip(&before[k][k..]) {
                *entry -= m * pivot;
            }
            b[i] -= m * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (b[i] - (i + 1..n).map(|j| a[i][j] * x[j]).sum::<f64>()) / a[i][i];
    }
    if x.iter().any(|v| !v.is_finite()) {
        return Err("DC grid step is nonfinite".into());
    }
    Ok(x)
}
fn norm(x: &[f64]) -> f64 {
    if x.iter().any(|v| !v.is_finite()) {
        return f64::INFINITY;
    }
    x.iter().map(|v| v.abs()).fold(0.0, f64::max)
}

/// Independent physical-unit checks allow the solver's 1e-8 relative bound
/// relaxation plus numerical evaluation error. The absolute floors are MW/kV
/// or kA, respectively; the relative allowance is only 0.02 parts per million.
fn limit_tolerance(bound: f64, absolute: f64) -> f64 {
    absolute + 2e-8 * bound.abs()
}

pub fn solve(grid: &DcGrid) -> Result<DcGridSolution, String> {
    solve_with_iterations(grid, 60)
}

fn solve_with_iterations(grid: &DcGrid, max_iterations: usize) -> Result<DcGridSolution, String> {
    let p = Prepared::new(grid)?;
    let n = grid.nodes.len();
    let mut x: Vec<_> = grid
        .nodes
        .iter()
        .map(|n| n.reference_kv.unwrap_or(n.initial_kv))
        .collect();
    x.extend(p.converters.iter().map(|&(a, b, c)| {
        match c.control {
            DcControl::ActivePower { mw } | DcControl::Droop { mw, .. } => {
                mw / (grid.nodes[a].initial_kv - grid.nodes[b].initial_kv)
                    .abs()
                    .max(1.0)
            }
            DcControl::Current { ka } => ka,
            DcControl::Voltage { .. } => 0.0,
        }
    }));
    let mut converged = None;
    for iteration in 0..=max_iterations {
        let (f, j) = p.equations(&x);
        let residual = norm(&f);
        if residual < 1e-9 {
            linear(j, vec![0.0; x.len()])?;
            converged = Some((iteration, residual));
            break;
        }
        if iteration == max_iterations {
            break;
        }
        let step = linear(j, f.iter().map(|v| -v).collect())?;
        let mut alpha = 1.0;
        let mut next = None;
        for _ in 0..24 {
            let trial: Vec<_> = x.iter().zip(&step).map(|(x, d)| x + alpha * d).collect();
            if p.converters.iter().all(|&(a, b, _)| trial[a] > trial[b])
                && norm(&p.equations(&trial).0) < residual
            {
                next = Some(trial);
                break;
            }
            alpha *= 0.5;
        }
        x = next.ok_or(
            "DC grid Newton line search failed; no acceptable positive converter-voltage step",
        )?;
    }
    let (iterations, residual) = converged
        .ok_or_else(|| format!("DC grid did not converge within {max_iterations} iterations"))?;
    let mut violations = Vec::new();
    let branches = p
        .branches
        .iter()
        .map(|&(a, b, l)| {
            let i = (x[a] - x[b]) / l.resistance_ohm;
            if l.current_limit_ka
                .is_some_and(|limit| i.abs() > limit + limit_tolerance(limit, 1e-8))
            {
                violations.push(format!("{} exceeds its current limit", l.id));
            }
            DcBranchResult {
                id: l.id.clone(),
                current_ka: i,
                loss_mw: i * i * l.resistance_ohm,
            }
        })
        .collect();
    let converters = p
        .converters
        .iter()
        .enumerate()
        .map(|(k, &(a, b, c))| {
            let u = x[a] - x[b];
            let i = x[n + k];
            let losses = loss(c, i).0;
            let pac = u * i + losses;
            if matches!(c.current_direction, Some(DcCurrentDirection::Positive)) && i < -1e-8
                || matches!(c.current_direction, Some(DcCurrentDirection::Negative)) && i > 1e-8
            {
                violations.push(format!("{} violates its converter current direction", c.id));
            }
            if c.current_limit_ka
                .is_some_and(|v| i.abs() > v + limit_tolerance(v, 1e-8))
            {
                violations.push(format!("{} exceeds its current limit", c.id));
            }
            if c.active_min_mw
                .is_some_and(|v| pac < v - limit_tolerance(v, 1e-7))
                || c.active_max_mw
                    .is_some_and(|v| pac > v + limit_tolerance(v, 1e-7))
            {
                violations.push(format!("{} exceeds its active-power range", c.id));
            }
            if c.voltage_min_kv
                .is_some_and(|v| u < v - limit_tolerance(v, 1e-7))
                || c.voltage_max_kv
                    .is_some_and(|v| u > v + limit_tolerance(v, 1e-7))
            {
                violations.push(format!("{} exceeds its DC-voltage range", c.id));
            }
            DcConverterResult {
                id: c.id.clone(),
                ac_bus: c.ac_bus,
                voltage_kv: u,
                current_ka: i,
                ac_power_mw: pac,
                loss_mw: losses,
            }
        })
        .collect();
    let limits_checked = p
        .branches
        .iter()
        .filter(|(_, _, b)| b.current_limit_ka.is_some())
        .count()
        + p.converters
            .iter()
            .map(|(_, _, c)| {
                [
                    c.current_limit_ka,
                    c.active_min_mw,
                    c.active_max_mw,
                    c.voltage_min_kv,
                    c.voltage_max_kv,
                ]
                .iter()
                .filter(|v| v.is_some())
                .count()
            })
            .sum::<usize>();
    let limits_unspecified = p.branches.len() + 5 * p.converters.len() - limits_checked;
    Ok(DcGridSolution {
        limits_checked: limits_checked
            + p.converters
                .iter()
                .filter(|(_, _, c)| c.current_direction.is_some())
                .count(),
        limits_unspecified,
        iterations,
        residual,
        nodes: grid
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| DcNodeResult {
                id: n.id.clone(),
                voltage_kv: x[i],
            })
            .collect(),
        branches,
        converters,
        violations,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    fn converter(id: &str, positive: &str, control: DcControl) -> DcConverter {
        DcConverter {
            id: id.into(),
            positive: positive.into(),
            negative: "return".into(),
            ac_bus: 1,
            in_service: true,
            control,
            current_direction: None,
            loss: [0.0; 3],
            current_limit_ka: None,
            active_min_mw: None,
            active_max_mw: None,
            voltage_min_kv: None,
            voltage_max_kv: None,
        }
    }
    #[test]
    fn rectifier_direction_is_not_inferred_from_positive_ac_power() {
        let mut grid = fixture();
        let c = &mut grid.converters[1];
        c.loss = [1.0, 0.0, 0.0];
        c.control = DcControl::ActivePower { mw: 0.1 };
        c.active_min_mw = Some(0.0);
        c.current_direction = Some(DcCurrentDirection::Positive);
        let r = solve(&grid).unwrap();
        assert!(r.violations.iter().any(|v| v.contains("current direction")));
        assert!(!r.violations.iter().any(|v| v.contains("active-power")));
    }
    #[test]
    fn limit_checks_accept_roundoff_but_report_physical_violations() {
        let mut grid = fixture();
        // Bind a 150 MW withdrawal with a small allowed NLP relaxation.
        grid.converters[1].control = DcControl::ActivePower {
            mw: -150.0 - 1.5e-6,
        };
        grid.converters[1].active_min_mw = Some(-150.0);
        assert!(!solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("active-power")));
        grid.converters[1].control = DcControl::ActivePower { mw: -150.001 };
        assert!(solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("active-power")));
        grid.converters[0].voltage_min_kv = Some(320.0 + 3e-6);
        assert!(!solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("DC-voltage")));
        grid.converters[0].voltage_min_kv = Some(320.001);
        assert!(solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("DC-voltage")));
        let branch_current = solve(&grid).unwrap().branches[0].current_ka.abs();
        grid.branches[0].current_limit_ka = Some(branch_current - 1e-9);
        assert!(!solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v == "ab exceeds its current limit"));
        grid.branches[0].current_limit_ka = Some(branch_current - 1e-4);
        assert!(solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v == "ab exceeds its current limit"));
        grid.branches[0].current_limit_ka = None;
        let current = solve(&grid).unwrap().converters[0].current_ka.abs();
        grid.converters[0].current_limit_ka = Some(current - 1e-9);
        assert!(!solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("current limit")));
        grid.converters[0].current_limit_ka = Some(current - 1e-4);
        assert!(solve(&grid)
            .unwrap()
            .violations
            .iter()
            .any(|v| v.contains("current limit")));
    }

    #[test]
    fn final_newton_step_is_checked() {
        let grid = fixture();
        let expected = solve(&grid).unwrap();
        assert!(expected.iterations > 0);
        let last = solve_with_iterations(&grid, expected.iterations).unwrap();
        assert_eq!(last.iterations, expected.iterations);
        assert!(last.residual < 1e-9);
        assert!(solve_with_iterations(&grid, expected.iterations - 1).is_err());
    }

    fn fixture() -> DcGrid {
        DcGrid {
            nodes: vec![
                DcNode {
                    id: "return".into(),
                    initial_kv: 0.0,
                    reference_kv: Some(0.0),
                },
                DcNode {
                    id: "a".into(),
                    initial_kv: 320.0,
                    reference_kv: None,
                },
                DcNode {
                    id: "b".into(),
                    initial_kv: 320.0,
                    reference_kv: None,
                },
            ],
            branches: vec![DcBranch {
                id: "ab".into(),
                from: "a".into(),
                to: "b".into(),
                resistance_ohm: 5.0,
                in_service: true,
                current_limit_ka: None,
            }],
            converters: vec![
                converter("source", "a", DcControl::Voltage { kv: 320.0 }),
                converter("load", "b", DcControl::ActivePower { mw: -100.0 }),
            ],
        }
    }
    pub(crate) fn imported_fixture() -> powerio::BalancedNetwork {
        crate::ir::balanced_module(
            crate::ir::deserialize_module(include_str!("../tests/data/transmission-mtdc.pio.json"))
                .unwrap(),
        )
        .unwrap()
        .into_value()
    }
    #[test]
    fn detailed_source_mapping_and_units_are_explicit() {
        let net = imported_fixture();
        let g = from_network(&net).unwrap();
        assert_eq!(
            g.converters.iter().map(|c| c.ac_bus).collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
        assert_eq!(g.converters[0].loss, [0.2, 0.1, 0.5]);
        let solution = solve(&g).unwrap();
        assert_eq!(solution.converters.len(), 3);
        let e = crate::equipment::project(&net).unwrap();
        assert_eq!(e.dc_topology.links.len(), 4);
        assert_eq!(e.dc_topology.converters[0].connected1, Some(true));
        assert!(e.dc_model.is_some());
        // Missing loss/control quantities must not become fabricated zeroes.
        let mut missing = net.clone();
        std::sync::Arc::make_mut(missing.detailed_connectivity_mut().as_mut().unwrap())
            .voltage_source_converters[0]
            .idle_loss_mw = None;
        assert!(from_network(&missing).unwrap_err().contains("idle loss"));
    }
    #[test]
    #[cfg(feature = "sensitivity")]
    fn detailed_converters_cannot_silently_disappear_from_ac_solves() {
        use crate::api::{Problem, SolveRequest};
        let net = imported_fixture();
        for formulation in [Problem::DcOpf, Problem::DcPf, Problem::AcPf] {
            let req = SolveRequest {
                formulation,
                ..Default::default()
            };
            let e = crate::api::solve_network(&net, &req).unwrap_err();
            assert!(e.contains("detailed DC converter"), "{e}");
            let text = crate::ir::serialize_module(&powerio::PioModule::new(net.clone())).unwrap();
            let e = crate::api::solve_module_json(
                &text,
                &serde_json::json!({"formulation": formulation}).to_string(),
            )
            .unwrap_err();
            assert!(e.contains("detailed DC converter"), "{e}");
        }
    }
    #[test]
    fn back_to_back_and_zero_current_sensitivity() {
        let mut g = fixture();
        g.branches.clear();
        g.nodes.pop();
        g.converters[1].positive = "a".into();
        let s = solve(&g).unwrap();
        assert!((s.converters[0].ac_power_mw - 100.0).abs() < 1e-8);
        g.converters[1].control = DcControl::ActivePower { mw: 0.0 };
        g.converters[0].loss[1] = 0.1;
        assert!(active_power_sensitivity(&g, "load")
            .unwrap_err()
            .contains("nondifferentiable"));
    }

    #[test]
    fn point_to_point_matches_quadratic_and_conserves_power() {
        let s = solve(&fixture()).unwrap();
        assert_eq!(s.limits_checked, 0);
        assert_eq!(s.limits_unspecified, 11);
        let vb = (320.0 + (320.0_f64.powi(2) - 4.0 * 5.0 * 100.0).sqrt()) / 2.0;
        assert!((s.nodes[2].voltage_kv - vb).abs() < 1e-7);
        assert!(
            (s.converters.iter().map(|c| c.ac_power_mw).sum::<f64>() - s.branches[0].loss_mw).abs()
                < 1e-7
        );
    }
    #[test]
    fn radial_and_meshed_three_terminal_grids_balance_losses() {
        for mesh in [false, true] {
            let mut g = fixture();
            g.nodes.push(DcNode {
                id: "c".into(),
                initial_kv: 320.0,
                reference_kv: None,
            });
            g.branches.push(DcBranch {
                id: "bc".into(),
                from: "b".into(),
                to: "c".into(),
                resistance_ohm: 10.0,
                in_service: true,
                current_limit_ka: Some(0.01),
            });
            if mesh {
                g.branches.push(DcBranch {
                    id: "ac".into(),
                    from: "a".into(),
                    to: "c".into(),
                    resistance_ohm: 8.0,
                    in_service: true,
                    current_limit_ka: None,
                });
            }
            g.converters
                .push(converter("wind", "c", DcControl::ActivePower { mw: 50.0 }));
            g.converters[0].loss = [0.2, 0.1, 0.5];
            let s = solve(&g).unwrap();
            assert_eq!(s.converters.len(), 3);
            assert!(
                (s.converters
                    .iter()
                    .map(|c| c.ac_power_mw - c.loss_mw)
                    .sum::<f64>()
                    - s.branches.iter().map(|b| b.loss_mw).sum::<f64>())
                .abs()
                    < 1e-7
            );
            assert!(!s.violations.is_empty());
        }
    }
    #[test]
    fn droop_and_reverse_transfer_are_solved() {
        let mut g = fixture();
        g.converters[0].control = DcControl::Droop {
            mw: 0.0,
            kv: 320.0,
            mw_per_kv: 20.0,
        };
        g.converters[1].control = DcControl::ActivePower { mw: 100.0 };
        let s = solve(&g).unwrap();
        assert!(s.converters[0].ac_power_mw < 0.0);
        assert!(s.converters[0].voltage_kv > 320.0);
        assert!(s.branches[0].current_ka < 0.0);
    }
    #[test]
    fn missing_voltage_control_and_outage_are_not_silent() {
        let mut g = fixture();
        g.converters[0].control = DcControl::ActivePower { mw: 100.0 };
        assert!(solve(&g).unwrap_err().contains("no voltage-regulating"));
        g = fixture();
        g.branches[0].in_service = false;
        assert!(solve(&g).is_err());
    }
    #[test]
    fn implicit_sensitivity_matches_central_difference() {
        let g = fixture();
        let derivative = active_power_sensitivity(&g, "load").unwrap();
        let mut hi = g.clone();
        hi.converters[1].control = DcControl::ActivePower { mw: -99.99 };
        let mut lo = g;
        lo.converters[1].control = DcControl::ActivePower { mw: -100.01 };
        let hi = solve(&hi).unwrap();
        let lo = solve(&lo).unwrap();
        for (k, (_, v)) in derivative.node_voltage_kv_per_mw.iter().enumerate() {
            assert!((v - (hi.nodes[k].voltage_kv - lo.nodes[k].voltage_kv) / 0.02).abs() < 1e-7);
        }
        for (k, (_, v)) in derivative.converter_power_mw_per_mw.iter().enumerate() {
            assert!(
                (v - (hi.converters[k].ac_power_mw - lo.converters[k].ac_power_mw) / 0.02).abs()
                    < 1e-7
            );
        }
    }

    #[test]
    fn reference_potentials_cannot_introduce_unreported_power() {
        let mut g = fixture();
        g.nodes[1].reference_kv = Some(320.0);
        assert!(solve(&g).unwrap_err().contains("unmodeled power source"));
    }

    #[test]
    fn conflicting_voltage_sources_fail() {
        let mut g = fixture();
        g.converters
            .push(converter("conflict", "a", DcControl::Voltage { kv: 310.0 }));
        assert!(solve(&g).unwrap_err().contains("singular"));
    }
}

/// Resolve an AC PCC only through declared terminal/bus mappings. Never guess a
/// numeric bus ID from a source equipment name.
pub fn ac_terminal_bus(
    net: &powerio::BalancedNetwork,
    reference: &powerio_tx::TerminalReference,
) -> Option<usize> {
    let d = net.detailed_connectivity().as_ref()?;
    let terminal = d.terminals.iter().find(|t| {
        t.equipment == reference.equipment && t.terminal == reference.terminal && t.connected
    })?;
    let mut buses = std::collections::BTreeSet::new();
    if let Some(bus) = &terminal.bus {
        for b in &d.bus_breaker_buses {
            if &b.component == bus {
                if let Some(id) = b.calculated_bus {
                    buses.insert(id.0);
                }
            }
        }
    }
    if let Some(node) = &terminal.node {
        for b in &d.calculated_buses {
            if b.voltage_level == terminal.voltage_level && b.nodes.contains(node) {
                buses.insert(b.calculated_bus.0);
            }
        }
    }
    if buses.len() == 1 {
        buses.first().copied()
    } else {
        None
    }
}

/// Build an explicitly qualified resistive VSC/LCC grid from detailed source data.
/// Import formats with insufficient terminal/control/loss data return a reason;
/// absence of data never becomes an invented default operating model.
pub fn from_network(net: &powerio::BalancedNetwork) -> Result<DcGrid, String> {
    use powerio_tx::AcDcConverterControlMode as Mode;
    let d = net
        .detailed_connectivity()
        .as_ref()
        .ok_or("No detailed DC topology is recorded")?;
    if !d.dc_switches.is_empty() || !d.dc_series_devices.is_empty() {
        return Err(
            "Detailed DC switches/series devices require an explicit reduction before this solve"
                .into(),
        );
    }
    if d.dc_nodes.is_empty()
        || (d.voltage_source_converters.is_empty() && d.line_commutated_converters.is_empty())
    {
        return Err("Detailed DC nodes and VSC converter records are required".into());
    }
    let node_id = |t: &powerio_tx::DcTerminal| -> Result<String, String> {
        if t.connected != Some(true) {
            return Err("A DC terminal has unknown or disconnected state".into());
        }
        t.dc_node
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| "A DC terminal has no physical node reference".into())
    };
    let mut nodes = Vec::new();
    for n in &d.dc_nodes {
        let initial = n
            .voltage_kv
            .or(n.nominal_voltage_kv)
            .ok_or_else(|| format!("DC node {} has no initial or nominal voltage", n.component))?;
        nodes.push(DcNode {
            id: n.component.to_string(),
            initial_kv: initial,
            reference_kv: None,
        });
    }
    for ground in &d.dc_grounds {
        if ground.resistance_ohm != Some(0.0) {
            return Err(
                "Only explicitly solid DC grounds are supported by this source adapter".into(),
            );
        }
        let id = node_id(&ground.dc_terminal)?;
        let node = nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or("DC ground refers to an unknown node")?;
        node.initial_kv = 0.0;
        node.reference_kv = Some(0.0);
    }
    if !nodes.iter().any(|n| n.reference_kv.is_some()) {
        return Err(
            "Detailed DC grid requires an explicit potential reference/solid ground".into(),
        );
    }
    let mut branches = Vec::new();
    for line in &d.dc_lines {
        let active = line.dc_terminal1.connected != Some(false)
            && line.dc_terminal2.connected != Some(false);
        // Preserve disconnected equipment in source presentation; it contributes
        // no conductance. Do not require its unused parameters to solve a live grid.
        if !active {
            continue;
        }
        branches.push(DcBranch {
            id: line.component.to_string(),
            from: node_id(&line.dc_terminal1)?,
            to: node_id(&line.dc_terminal2)?,
            resistance_ohm: line
                .resistance_ohm
                .ok_or("DC line resistance is not specified")?,
            in_service: true,
            current_limit_ka: None,
        });
    }
    let mut converters = Vec::new();
    for c in &d.voltage_source_converters {
        if c.dc_terminal1.connected == Some(false) || c.dc_terminal2.connected == Some(false) {
            continue;
        }
        if c.dc_terminal1.polarity != Some(powerio_tx::DcPolarity::Positive)
            || !matches!(
                c.dc_terminal2.polarity,
                Some(powerio_tx::DcPolarity::Negative | powerio_tx::DcPolarity::Middle)
            )
        {
            return Err(format!(
                "Converter {} needs explicit positive and negative/return terminal polarity",
                c.component
            ));
        }
        if c.maximum_valve_current_a.is_some() {
            return Err(
                "Valve-current limits require a converter-specific AC/DC current relation".into(),
            );
        }
        let control = match c.control_mode {
            Some(Mode::DcVoltage) => DcControl::Voltage{kv:c.target_dc_voltage_kv.ok_or("DC voltage control has no target")?},
            Some(Mode::ActivePowerAtPcc) => DcControl::ActivePower{mw:c.target_active_power_mw.ok_or("DC converter active-power control has no target")?},
            _ => return Err(format!("Converter {} has an unsupported or unspecified source control; droop constants require explicit unit conversion",c.component)),
        };
        let ac_bus = c
            .pcc_terminal
            .as_ref()
            .and_then(|t| ac_terminal_bus(net, t))
            .ok_or_else(|| {
                format!(
                    "Converter {} has no unambiguous AC PCC mapping",
                    c.component
                )
            })?;
        let a = c
            .idle_loss_mw
            .ok_or("Converter idle loss is not specified")?;
        let b = c
            .switching_loss_mw_per_ampere
            .ok_or("Converter switching loss is not specified")?
            * 1000.0;
        let resistance = c
            .resistive_loss_ohm
            .ok_or("Converter resistive loss is not specified")?;
        converters.push(DcConverter {
            id: c.component.to_string(),
            positive: node_id(&c.dc_terminal1)?,
            negative: node_id(&c.dc_terminal2)?,
            ac_bus,
            in_service: true,
            control,
            current_direction: None,
            loss: [a, b, resistance],
            current_limit_ka: None,
            active_min_mw: c.minimum_active_power_mw,
            active_max_mw: c.maximum_active_power_mw,
            voltage_min_kv: c.minimum_dc_voltage_kv,
            voltage_max_kv: c.maximum_dc_voltage_kv,
        });
    }
    for c in &d.line_commutated_converters {
        if c.dc_terminal1.connected == Some(false) || c.dc_terminal2.connected == Some(false) {
            continue;
        }
        crate::ac_dc::lcc_power_factor(c)?;
        if c.dc_terminal1.polarity != Some(powerio_tx::DcPolarity::Positive)
            || !matches!(
                c.dc_terminal2.polarity,
                Some(powerio_tx::DcPolarity::Negative | powerio_tx::DcPolarity::Middle)
            )
        {
            return Err(format!(
                "LCC {} needs explicit terminal polarity",
                c.component
            ));
        }
        let control = match c.control_mode {
            Some(Mode::DcVoltage) => DcControl::Voltage {
                kv: c
                    .target_dc_voltage_kv
                    .ok_or("LCC DC voltage target missing")?,
            },
            Some(Mode::ActivePowerAtPcc) => DcControl::ActivePower {
                mw: c
                    .target_active_power_mw
                    .ok_or("LCC active target missing")?,
            },
            Some(Mode::DcCurrent) => DcControl::Current {
                ka: c.target_dc_current_a.ok_or("LCC current target missing")? / 1000.0,
            },
            _ => return Err("Unsupported LCC DC control".into()),
        };
        let ac_bus = c
            .pcc_terminal
            .as_ref()
            .and_then(|t| ac_terminal_bus(net, t))
            .ok_or("LCC AC mapping missing")?;
        let rectifier =
            c.operating_mode == Some(powerio_tx::LineCommutatedConverterOperatingMode::Rectifier);
        converters.push(DcConverter {
            id: c.component.to_string(),
            positive: node_id(&c.dc_terminal1)?,
            negative: node_id(&c.dc_terminal2)?,
            ac_bus,
            in_service: true,
            control,
            current_direction: Some(if rectifier {
                DcCurrentDirection::Positive
            } else {
                DcCurrentDirection::Negative
            }),
            loss: [
                c.idle_loss_mw.ok_or("LCC idle loss missing")?,
                1000.0
                    * c.switching_loss_mw_per_ampere
                        .ok_or("LCC switching loss missing")?,
                c.resistive_loss_ohm.ok_or("LCC resistive loss missing")?,
            ],
            current_limit_ka: None,
            active_min_mw: if rectifier {
                Some(c.minimum_active_power_mw.unwrap_or(0.0).max(0.0))
            } else {
                c.minimum_active_power_mw
            },
            active_max_mw: if rectifier {
                c.maximum_active_power_mw
            } else {
                Some(c.maximum_active_power_mw.unwrap_or(0.0).min(0.0))
            },
            voltage_min_kv: c.minimum_dc_voltage_kv,
            voltage_max_kv: c.maximum_dc_voltage_kv,
        });
    }
    let grid = DcGrid {
        nodes,
        branches,
        converters,
    };
    Prepared::new(&grid)?;
    Ok(grid)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DcGridSensitivity {
    pub converter: String,
    pub node_voltage_kv_per_mw: Vec<(String, f64)>,
    pub converter_power_mw_per_mw: Vec<(String, f64)>,
}

/// Exact implicit derivative with respect to one active-power/droop target.
/// Ratings are observations in power flow; this is not a derivative through
/// saturation, switching, or a change of converter control mode.
pub fn active_power_sensitivity(
    grid: &DcGrid,
    converter: &str,
) -> Result<DcGridSensitivity, String> {
    let result = solve(grid)?;
    let p = Prepared::new(grid)?;
    if p.converters
        .iter()
        .zip(&result.converters)
        .any(|((_, _, c), r)| c.loss[1] > 0.0 && r.current_ka.abs() < 1e-10)
    {
        return Err("Converter absolute-current loss is nondifferentiable at zero current".into());
    }
    let n = grid.nodes.len();
    let mut x: Vec<_> = result.nodes.iter().map(|n| n.voltage_kv).collect();
    x.extend(result.converters.iter().map(|c| c.current_ka));
    let k = p
        .converters
        .iter()
        .position(|(_, _, c)| c.id == converter)
        .ok_or("Unknown or out-of-service DC converter")?;
    let (a, b, c) = p.converters[k];
    if !matches!(
        c.control,
        DcControl::ActivePower { .. } | DcControl::Droop { .. }
    ) {
        return Err("Converter has no active-power target".into());
    }
    let scale = (grid.nodes[a].initial_kv - grid.nodes[b].initial_kv)
        .abs()
        .max(1.0);
    let (_, j) = p.equations(&x);
    let mut rhs = vec![0.0; x.len()];
    rhs[n + k] = 1.0 / scale;
    let dx = linear(j, rhs)?;
    let powers = p
        .converters
        .iter()
        .enumerate()
        .map(|(k, &(a, b, c))| {
            let i = x[n + k];
            let u = x[a] - x[b];
            (
                c.id.clone(),
                i * (dx[a] - dx[b]) + (u + loss(c, i).1) * dx[n + k],
            )
        })
        .collect();
    Ok(DcGridSensitivity {
        converter: converter.to_owned(),
        node_voltage_kv_per_mw: grid
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), dx[i]))
            .collect(),
        converter_power_mw_per_mw: powers,
    })
}
