//! Finite-difference parity for the differentiable sensitivities (`book/src/methodology.md`),
//! the second independent baseline. For each sampled `(operand, parameter)` cell we
//! check two things, mirroring the in-tree `check_parity` / `check_conic_parity`:
//!
//! 1. **adjoint == forward** on every supported cell — a solve-consistency bound. The
//!    two directions are algebraically identical; the gap is just the regularized
//!    solve's floating point (~1e-11 stiff, ~1e-4 on the Jabr cone's soft directions).
//! 2. **central finite difference vs the analytic column** — re-solve at `±ε`, read the
//!    operand off the perturbed solution, compare to the analytic column. Columns below
//!    the regularization floor carry no resolvable derivative and are skipped.
//!
//! AC and conic go through the typed engines (`AcNewton` / `ConicKkt` + `sensitivity`),
//! perturbing the public network fields. DC goes through `sensitivity_json` (the only
//! external route to DC sensitivities) with the demand FD driven through `solve_network`.

use powerio::network::Network;
use serde_json::Value;
use tellegen::{
    ac_pf, acopf, sensitivity, socwr_opf, solve_json, AcNetwork, AcNewton, AcOpfKkt, AcOpfSolution,
    AcPfSolution, AcPolar, Bound, ConicKkt, CostTerm, Differentiable, Edits, End, Mode, Operand,
    Parameter, Power, SocWrSolution, SolveRequest, VoltageKind, GB,
};

use crate::record::ParitySummary;

const ZERO_FLOOR: f64 = 5e-3;
const FLOOR_FRAC: f64 = 1e-2;
/// At most this many parameter columns are sampled per cell, evenly spaced. Bounds
/// the analytic dense solve and the FD re-solves on large cases.
const MAX_COLS: usize = 6;

fn l2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Evenly-spaced indices into `0..len`, at most `k`.
fn sample_indices(len: usize, k: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    if len <= k {
        return (0..len).collect();
    }
    (0..k).map(|i| i * len / k).collect()
}

/// Jabr-coupled / soft cells take the looser tolerance (the in-tree classification):
/// a soft operand (squared voltage, a reactive injection, the reactive price) or a
/// conductance (loss-direction) parameter.
fn soft_cell(op: Operand, par: Parameter) -> bool {
    let soft_op = matches!(
        op,
        Operand::Voltage(VoltageKind::Squared)
            | Operand::Dispatch(Power::Reactive)
            | Operand::Flow {
                power: Power::Reactive,
                ..
            }
            | Operand::Price(Power::Reactive)
    );
    let soft_par = matches!(
        par,
        Parameter::SeriesAdmittance(GB::Conductance) | Parameter::ShuntAdmittance(GB::Conductance)
    );
    soft_op || soft_par
}

/// Per-parameter FD step. Small enough to hold the active set, large enough to clear
/// the 1e-9 solver tolerance — the scales the in-tree conic tests use.
fn eps_for(par: Parameter) -> f64 {
    match par {
        Parameter::Demand(_) | Parameter::LineLimit | Parameter::GenBound { .. } => 1e-3,
        Parameter::Cost(_) => 1e-1,
        _ => 1e-4,
    }
}

// --- Conic operand readout (public SocWrSolution; reactive price needs in-crate state) -

fn conic_operand(sol: &SocWrSolution, op: Operand) -> Option<Vec<f64>> {
    Some(match op {
        Operand::Dispatch(Power::Active) => sol.pg.clone(),
        Operand::Dispatch(Power::Reactive) => sol.qg.clone(),
        Operand::Price(Power::Active) => sol.lmp.clone(),
        Operand::Voltage(VoltageKind::Squared) => sol.w.clone(),
        Operand::Voltage(VoltageKind::ProductReal) => sol.wr.clone(),
        Operand::Voltage(VoltageKind::ProductImag) => sol.wi.clone(),
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        } => sol.pf.clone(),
        Operand::Flow {
            power: Power::Active,
            end: End::To,
        } => sol.pt.clone(),
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        } => sol.qf.clone(),
        Operand::Flow {
            power: Power::Reactive,
            end: End::To,
        } => sol.qt.clone(),
        // Price(Reactive) is `−z[r_qbal]`, which needs the conic layout and the
        // crate-private raw dual; not readable from the public solution.
        _ => return None,
    })
}

fn ac_operand(sol: &AcPfSolution, op: Operand) -> Option<Vec<f64>> {
    Some(match op {
        Operand::Voltage(VoltageKind::Magnitude) => sol.vm.clone(),
        Operand::Voltage(VoltageKind::Angle) => sol.va.clone(),
        _ => return None,
    })
}

fn acopf_operand(sol: &AcOpfSolution, op: Operand) -> Option<Vec<f64>> {
    Some(match op {
        Operand::Voltage(VoltageKind::Magnitude) => sol.vm.clone(),
        Operand::Voltage(VoltageKind::Angle) => sol.va.clone(),
        Operand::Dispatch(Power::Active) => sol.pg.clone(),
        Operand::Dispatch(Power::Reactive) => sol.qg.clone(),
        Operand::Price(Power::Active) => sol.lmp.clone(),
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        } => sol.pf.clone(),
        Operand::Flow {
            power: Power::Active,
            end: End::To,
        } => sol.pt.clone(),
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        } => sol.qf.clone(),
        Operand::Flow {
            power: Power::Reactive,
            end: End::To,
        } => sol.qt.clone(),
        _ => return None,
    })
}

/// A copy of `net` with the `idx`-th value of `par` shifted by `d`, for the parameters
/// the corpus parity sweep perturbs. Mirrors the in-tree conic `perturb`.
fn perturb_ac(net: &AcNetwork, par: Parameter, idx: usize, d: f64) -> Option<AcNetwork> {
    let mut n = net.clone();
    match par {
        Parameter::Demand(Power::Active) => n.pd[idx] += d,
        Parameter::Demand(Power::Reactive) => n.qd[idx] += d,
        Parameter::LineLimit => n.rate_a[idx] += d,
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Max,
        } => n.pmax[idx] += d,
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Min,
        } => n.pmin[idx] += d,
        Parameter::GenBound {
            power: Power::Reactive,
            bound: Bound::Max,
        } => n.qmax[idx] += d,
        Parameter::GenBound {
            power: Power::Reactive,
            bound: Bound::Min,
        } => n.qmin[idx] += d,
        Parameter::Cost(CostTerm::Quadratic) => n.cq[idx] += d,
        Parameter::Cost(CostTerm::Linear) => n.cl[idx] += d,
        Parameter::SeriesAdmittance(GB::Conductance) => n.g[idx] += d,
        Parameter::SeriesAdmittance(GB::Susceptance) => n.b[idx] += d,
        Parameter::ShuntAdmittance(GB::Conductance) => n.gs[idx] += d,
        Parameter::ShuntAdmittance(GB::Susceptance) => n.bs[idx] += d,
        _ => return None,
    }
    Some(n)
}

/// Collect one FD relative error into the right parity-class bucket of `sum`. The
/// worst / median scalars are reduced from these by `ParitySummary::finalize`.
fn record_rel(sum: &mut ParitySummary, op: Operand, par: Parameter, rel: f64) {
    if soft_cell(op, par) {
        sum.coupled_errs.push(rel);
    } else {
        sum.clean_errs.push(rel);
    }
}

/// The conic candidate cells: enough to touch each parity class. The demand/cost cells
/// are finite-differenced broadly; the binding-constraint cells (line limit, gen bound)
/// are probed for support and adjoint/forward consistency, and finite-differenced only
/// where a sampled index happens to bind (the in-tree fixtures pin the binding case).
const CONIC_CELLS: &[(Operand, Parameter)] = &[
    (
        Operand::Price(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Cost(CostTerm::Linear),
    ),
    (
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        },
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Reactive),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Voltage(VoltageKind::Squared),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        },
        Parameter::Demand(Power::Reactive),
    ),
    (Operand::Price(Power::Active), Parameter::LineLimit),
    (
        Operand::Dispatch(Power::Active),
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Max,
        },
    ),
    (
        Operand::Price(Power::Reactive),
        Parameter::Demand(Power::Active),
    ),
];

const AC_CELLS: &[(Operand, Parameter)] = &[
    (
        Operand::Voltage(VoltageKind::Angle),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Voltage(VoltageKind::Magnitude),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Voltage(VoltageKind::Magnitude),
        Parameter::Demand(Power::Active),
    ),
];

/// AC OPF candidate cells. `AcOpfKkt` supports the Demand and Cost parameters; sample the
/// price, dispatch, and voltage operands against them.
const ACOPF_CELLS: &[(Operand, Parameter)] = &[
    (
        Operand::Price(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Voltage(VoltageKind::Magnitude),
        Parameter::Demand(Power::Reactive),
    ),
    (
        Operand::Voltage(VoltageKind::Angle),
        Parameter::Demand(Power::Active),
    ),
    (
        Operand::Dispatch(Power::Active),
        Parameter::Cost(CostTerm::Linear),
    ),
];

/// Conic FD parity for one network. Solves SOCWR once, builds the conic KKT, then
/// probes each candidate cell.
pub fn conic_parity(net: &AcNetwork) -> ParitySummary {
    let mut sum = ParitySummary::new("socwr");
    let sol = match socwr_opf(net) {
        Ok(s) => s,
        Err(e) => {
            sum.notes.push(format!("socwr solve failed: {e}"));
            return sum;
        }
    };
    let sys = match ConicKkt::new(net, &sol) {
        Ok(s) => s,
        Err(e) => {
            sum.notes.push(format!("conic kkt build failed: {e}"));
            return sum;
        }
    };

    for &(op, par) in CONIC_CELLS {
        sum.cells_probed += 1;
        let plen = match sys.parameter_len(par) {
            Some(n) if n > 0 => n,
            _ => continue, // unsupported parameter for this formulation
        };
        let idxs = sample_indices(plen, MAX_COLS);
        let fwd = match sensitivity(&sys, op, par, Some(&idxs), Mode::Forward) {
            Ok(m) => m,
            Err(_) => continue, // unsupported (operand, parameter) cell
        };
        sum.cells_supported += 1;

        // adjoint == forward on the same sampled columns.
        if let Ok(adj) = sensitivity(&sys, op, par, Some(&idxs), Mode::Adjoint) {
            for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                for (a, b) in rf.iter().zip(ra.iter()) {
                    sum.worst_adjoint_forward = sum.worst_adjoint_forward.max((a - b).abs());
                }
            }
        }

        // FD the significant columns, if the operand is readable from the public solution.
        if conic_operand(&sol, op).is_none() {
            continue;
        }
        let ncols = fwd.cols.len();
        let nrows = fwd.values.len();
        let col = |c: usize| (0..nrows).map(|r| fwd.values[r][c]).collect::<Vec<_>>();
        let norms: Vec<f64> = (0..ncols).map(|c| l2(&col(c))).collect();
        let man = norms.iter().cloned().fold(0.0, f64::max);
        if man < ZERO_FLOOR {
            continue; // every sampled column slack for this cell
        }
        let floor = FLOOR_FRAC * man;
        let eps = eps_for(par);
        // `c` jointly indexes `norms`, the analytic column, and the dense parameter index.
        #[allow(clippy::needless_range_loop)]
        for c in 0..ncols {
            if norms[c] < floor {
                continue;
            }
            let dense = fwd.cols[c].index;
            let (Some(sp), Some(sm)) = (
                perturb_ac(net, par, dense, eps).and_then(|n| socwr_opf(&n).ok()),
                perturb_ac(net, par, dense, -eps).and_then(|n| socwr_opf(&n).ok()),
            ) else {
                continue; // a perturbed solve failed; skip this column
            };
            let (Some(opp), Some(opm)) = (conic_operand(&sp, op), conic_operand(&sm, op)) else {
                continue;
            };
            let an = col(c);
            // Align the FD readout to the analytic rows: row i reports element index
            // `fwd.rows[i].index`, which is not 0..len when the operand spans a subset.
            let diff: Vec<f64> = (0..an.len())
                .map(|i| {
                    let e = fwd.rows[i].index;
                    an[i] - (opp[e] - opm[e]) / (2.0 * eps)
                })
                .collect();
            sum.fd_columns += 1;
            record_rel(&mut sum, op, par, l2(&diff) / norms[c]);
        }
    }
    sum
}

/// AC power flow FD parity for one network. The operands are bus voltages; the
/// parameters demand. Non-convergence at the perturbed point drops that column.
pub fn ac_parity(net: &AcNetwork) -> ParitySummary {
    let mut sum = ParitySummary::new("ac");
    let sol = match ac_pf(&AcPolar::new(), net) {
        Ok(s) => s,
        Err(e) => {
            sum.notes.push(format!("ac power flow failed: {e}"));
            return sum;
        }
    };
    let sys = AcNewton::new(net, &sol);

    for &(op, par) in AC_CELLS {
        sum.cells_probed += 1;
        let plen = match sys.parameter_len(par) {
            Some(n) if n > 0 => n,
            _ => continue,
        };
        let idxs = sample_indices(plen, MAX_COLS);
        let fwd = match sensitivity(&sys, op, par, Some(&idxs), Mode::Forward) {
            Ok(m) => m,
            Err(_) => continue,
        };
        sum.cells_supported += 1;
        if let Ok(adj) = sensitivity(&sys, op, par, Some(&idxs), Mode::Adjoint) {
            for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                for (a, b) in rf.iter().zip(ra.iter()) {
                    sum.worst_adjoint_forward = sum.worst_adjoint_forward.max((a - b).abs());
                }
            }
        }
        if ac_operand(&sol, op).is_none() {
            continue;
        }
        let nrows = fwd.values.len();
        let col = |c: usize| (0..nrows).map(|r| fwd.values[r][c]).collect::<Vec<_>>();
        let norms: Vec<f64> = (0..fwd.cols.len()).map(|c| l2(&col(c))).collect();
        let man = norms.iter().cloned().fold(0.0, f64::max);
        if man < ZERO_FLOOR {
            continue;
        }
        let floor = FLOOR_FRAC * man;
        let eps = eps_for(par);
        // `c` jointly indexes `norms`, the analytic column, and the dense parameter index.
        #[allow(clippy::needless_range_loop)]
        for c in 0..fwd.cols.len() {
            if norms[c] < floor {
                continue;
            }
            let dense = fwd.cols[c].index;
            let (Some(sp), Some(sm)) = (
                perturb_ac(net, par, dense, eps).and_then(|n| ac_pf(&AcPolar::new(), &n).ok()),
                perturb_ac(net, par, dense, -eps).and_then(|n| ac_pf(&AcPolar::new(), &n).ok()),
            ) else {
                continue;
            };
            let (Some(opp), Some(opm)) = (ac_operand(&sp, op), ac_operand(&sm, op)) else {
                continue;
            };
            let an = col(c);
            // Row i reports bus `fwd.rows[i].index`; the AC voltage operand spans the
            // free buses, not 0..n, so the FD readout must use that dense index.
            let diff: Vec<f64> = (0..an.len())
                .map(|i| {
                    let e = fwd.rows[i].index;
                    an[i] - (opp[e] - opm[e]) / (2.0 * eps)
                })
                .collect();
            sum.fd_columns += 1;
            record_rel(&mut sum, op, par, l2(&diff) / norms[c]);
        }
    }
    sum
}

/// Full AC OPF FD parity for one network. Mirrors [`ac_parity`]: solve the exact AC OPF
/// once, build the KKT, probe each cell for adjoint/forward consistency, and finite-
/// difference the significant columns by perturbing the public network fields. The AC OPF
/// re-solves are the heaviest in the sweep, so this is gated by `--max-sens-bus` like the
/// others.
pub fn acopf_parity(net: &AcNetwork) -> ParitySummary {
    let mut sum = ParitySummary::new("acopf");
    let sol = match acopf(net) {
        Ok(s) => s,
        Err(e) => {
            sum.notes.push(format!("ac opf solve failed: {e}"));
            return sum;
        }
    };
    let sys = match AcOpfKkt::new(net, &sol) {
        Ok(s) => s,
        Err(e) => {
            sum.notes.push(format!("ac opf kkt build failed: {e}"));
            return sum;
        }
    };

    for &(op, par) in ACOPF_CELLS {
        sum.cells_probed += 1;
        let plen = match sys.parameter_len(par) {
            Some(n) if n > 0 => n,
            _ => continue,
        };
        let idxs = sample_indices(plen, MAX_COLS);
        let fwd = match sensitivity(&sys, op, par, Some(&idxs), Mode::Forward) {
            Ok(m) => m,
            Err(_) => continue,
        };
        sum.cells_supported += 1;
        if let Ok(adj) = sensitivity(&sys, op, par, Some(&idxs), Mode::Adjoint) {
            for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                for (a, b) in rf.iter().zip(ra.iter()) {
                    sum.worst_adjoint_forward = sum.worst_adjoint_forward.max((a - b).abs());
                }
            }
        }
        if acopf_operand(&sol, op).is_none() {
            continue;
        }
        let nrows = fwd.values.len();
        let col = |c: usize| (0..nrows).map(|r| fwd.values[r][c]).collect::<Vec<_>>();
        let norms: Vec<f64> = (0..fwd.cols.len()).map(|c| l2(&col(c))).collect();
        let man = norms.iter().cloned().fold(0.0, f64::max);
        if man < ZERO_FLOOR {
            continue;
        }
        let floor = FLOOR_FRAC * man;
        let eps = eps_for(par);
        // `c` jointly indexes `norms`, the analytic column, and the dense parameter index.
        #[allow(clippy::needless_range_loop)]
        for c in 0..fwd.cols.len() {
            if norms[c] < floor {
                continue;
            }
            let dense = fwd.cols[c].index;
            let (Some(sp), Some(sm)) = (
                perturb_ac(net, par, dense, eps).and_then(|n| acopf(&n).ok()),
                perturb_ac(net, par, dense, -eps).and_then(|n| acopf(&n).ok()),
            ) else {
                continue;
            };
            let (Some(opp), Some(opm)) = (acopf_operand(&sp, op), acopf_operand(&sm, op)) else {
                continue;
            };
            let an = col(c);
            let diff: Vec<f64> = (0..an.len())
                .map(|i| {
                    let e = fwd.rows[i].index;
                    an[i] - (opp[e] - opm[e]) / (2.0 * eps)
                })
                .collect();
            sum.fd_columns += 1;
            record_rel(&mut sum, op, par, l2(&diff) / norms[c]);
        }
    }
    sum
}

/// DC dLMP/dd FD parity through the public JSON sensitivity endpoint (the only external
/// route to DC sensitivities). The analytic `d(price)/d(demand)` column is compared to a
/// central finite difference taken through `solve_network`'s demand deltas, both in the
/// served `($/MWh)/MW`.
pub fn dc_parity(net: &Network) -> ParitySummary {
    let mut sum = ParitySummary::new("dc");
    sum.cells_probed = 1;
    let Ok(network_json) = net.to_json() else {
        sum.notes.push("network to_json failed".into());
        return sum;
    };

    // Sample a handful of demand buses (dense indices), analytic only over those.
    let base = match tellegen::solve_network(net, &SolveRequest::default()) {
        Ok(o) => o,
        Err(e) => {
            sum.notes.push(format!("dc base solve failed: {e}"));
            return sum;
        }
    };
    let nbus = base.lmp.as_ref().map_or(0, |l| l.len());
    let idxs = sample_indices(nbus, MAX_COLS);
    let idx_json = serde_json::to_string(&idxs).unwrap_or_else(|_| "null".into());
    // The DC sensitivity now rides the unified solve endpoint: a dcopf solve carrying
    // one (Price, Demand) sensitivity cell, read back from `sensitivities[0]`.
    let req = format!(
        r#"{{"formulation":"dcopf","sensitivities":[{{"operand":{{"Price":"Active"}},"parameter":{{"Demand":"Active"}},"indices":{idx_json},"mode":"Forward"}}]}}"#
    );
    let cell = |json: &str| -> Option<Value> {
        let resp: Value = serde_json::from_str(&solve_json(&network_json, json).ok()?).ok()?;
        resp.get("sensitivities").and_then(|s| s.get(0)).cloned()
    };
    let m: Value = match cell(&req) {
        Some(m) => m,
        None => {
            sum.notes.push("dc solve_json sensitivity failed".into());
            return sum;
        }
    };
    sum.cells_supported = 1;

    // adjoint == forward through the same endpoint.
    let req_adj = req.replace("\"Forward\"", "\"Adjoint\"");
    if let Some(adj) = cell(&req_adj) {
        sum.worst_adjoint_forward = matrix_max_abs_diff(&m, &adj);
    }

    // Row order: price at each bus, named by original bus id. Build a bus-id → row map.
    let rows = m["rows"].as_array().cloned().unwrap_or_default();
    let row_bus: Vec<i64> = rows
        .iter()
        .map(|r| r["element"]["Bus"].as_i64().unwrap_or(-1))
        .collect();
    let cols = m["cols"].as_array().cloned().unwrap_or_default();
    let values = m["values"].as_array().cloned().unwrap_or_default();

    // Lookup from bus id to the lmp value in a SolveResponse.
    let lmp_by_bus = |out: &tellegen::SolveResponse, bus: i64| -> f64 {
        out.lmp
            .as_ref()
            .and_then(|l| l.iter().find(|v| v.bus as i64 == bus))
            .map(|v| v.value)
            .unwrap_or(0.0)
    };

    // Per sampled column: analytic vs central FD of lmp w.r.t. a ±1 MW demand step.
    let analytic_col = |c: usize| -> Vec<f64> {
        values
            .iter()
            .map(|row| {
                row.as_array()
                    .and_then(|r| r.get(c))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            })
            .collect()
    };
    let norms: Vec<f64> = (0..cols.len()).map(|c| l2(&analytic_col(c))).collect();
    let man = norms.iter().cloned().fold(0.0, f64::max);
    let floor = (FLOOR_FRAC * man).max(1e-6);
    const STEP_MW: f64 = 1.0;
    for c in 0..cols.len() {
        if norms[c] < floor {
            continue;
        }
        let bus = cols[c]["element"]["Bus"].as_i64().unwrap_or(-1);
        if bus < 0 {
            continue;
        }
        // The analytic path (sensitivity_json with no `shed` field) solves shed-off, so the
        // FD must too, to compare like for like. A ±1 MW step that tips the case unservable
        // fails the shed-off solve and that column is skipped below.
        let plus = SolveRequest {
            edits: Edits {
                deltas: [(bus, STEP_MW)].into_iter().collect(),
            },
            ..Default::default()
        };
        let minus = SolveRequest {
            edits: Edits {
                deltas: [(bus, -STEP_MW)].into_iter().collect(),
            },
            ..Default::default()
        };
        let (Ok(op), Ok(om)) = (
            tellegen::solve_network(net, &plus),
            tellegen::solve_network(net, &minus),
        ) else {
            continue;
        };
        let fd: Vec<f64> = row_bus
            .iter()
            .map(|&b| (lmp_by_bus(&op, b) - lmp_by_bus(&om, b)) / (2.0 * STEP_MW))
            .collect();
        let an = analytic_col(c);
        let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
        sum.fd_columns += 1;
        // dLMP/dd is an active power routed cell — FdClean.
        sum.clean_errs.push(l2(&diff) / norms[c]);
    }
    sum
}

/// Largest absolute entrywise difference between two SensitivityMatrix JSON values.
fn matrix_max_abs_diff(a: &Value, b: &Value) -> f64 {
    let (Some(va), Some(vb)) = (a["values"].as_array(), b["values"].as_array()) else {
        return 0.0;
    };
    let mut worst = 0.0f64;
    for (ra, rb) in va.iter().zip(vb.iter()) {
        let (Some(ra), Some(rb)) = (ra.as_array(), rb.as_array()) else {
            continue;
        };
        for (x, y) in ra.iter().zip(rb.iter()) {
            if let (Some(x), Some(y)) = (x.as_f64(), y.as_f64()) {
                worst = worst.max((x - y).abs());
            }
        }
    }
    worst
}
