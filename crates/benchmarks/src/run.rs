//! Per case runner: drive Tellegen's public API over one corpus file, time each
//! stage, compare against the BASELINE reference, and check sensitivities with
//! finite differences. The bus limit and per case timeout bound each run; every
//! skip is recorded with its reason.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use powerio::{AcOpfInstance, AcPfInstance, BalancedNetwork, DcOpfInstance, PioModule};
use tellegen::{
    solve_ac_instance, solve_ac_pf_instance, solve_instance, Iterations, Problem, SolveRequest,
};

use crate::baseline::BaselineRow;
use crate::corpus::CaseFile;
use crate::parity;
use crate::record::{Record, Repro, Status};

/// A DC objective within this relative tolerance of the published value reproduces it.
const DC_MATCH_TOL: f64 = 1e-2;
/// The SOCWR reproduces the published SOC relaxation when its gap is within this many
/// percentage points of the published SOC gap (the same Jabr relaxation family).
const SOC_GAP_TOL: f64 = 0.5;

/// Run-wide knobs (`docs/src/methodology.md`).
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Skip a case whose filename bus count exceeds this (0 = unlimited).
    pub max_bus: usize,
    /// Skip sensitivity sampling above this bus count (the dense solve is the memory
    /// bottleneck), while still solving the OPF/PF.
    pub max_sens_bus: usize,
    /// Per case wall time limit.
    pub timeout: Duration,
    /// Whether to check the sensitivities with finite differences.
    pub sample_sensitivity: bool,
}

/// Parse MATPOWER text in memory through the PowerIO module route. The
/// benchmark retains the typed value as the source for every declared problem.
fn parse_matpower(text: &str, name: &str) -> Result<PioModule<BalancedNetwork>, String> {
    let format = powerio::format::format_id_for("matpower").map_err(|e| e.to_string())?;
    let source = powerio::Source::from_bytes(name, text.as_bytes().to_vec())
        .map_err(|e| e.to_string())?
        .with_format(format);
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    let module: powerio::PioModule<powerio::BalancedNetwork> = powerio::try_into_typed(module)
        .map_err(|mismatch| format!("parsed a {} value", mismatch.actual().as_str()))?;
    Ok(module)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Run one case under a timeout. The work runs on a detached thread so a hung solve can be
/// abandoned, but the thread is not cancelled: a timed out solve keeps running and holding
/// its working set until it finishes on its own. Cases run in increasing size order, so an
/// abandoned solve does not perturb the small-case timings, yet its memory is not reclaimed
/// until it returns — size `--max-*-bus` to skip the giants that would actually leak rather
/// than relying on the timeout. The filename cap short-circuits before any thread is spawned.
pub fn run_case(cf: &CaseFile, baseline: Option<BaselineRow>, cfg: Config) -> Record {
    if cfg.max_bus != 0 && cf.buses > cfg.max_bus {
        return Record::skipped(
            cf,
            format!("buses {} exceed --max-bus {}", cf.buses, cfg.max_bus),
        );
    }
    let (tx, rx) = mpsc::channel();
    let cf2 = cf.clone();
    std::thread::spawn(move || {
        let _ = tx.send(run_case_inner(&cf2, baseline, cfg));
    });
    match rx.recv_timeout(cfg.timeout) {
        Ok(rec) => rec,
        Err(_) => Record::skipped(
            cf,
            format!("timed out after {:.0}s", cfg.timeout.as_secs_f64()),
        ),
    }
}

fn run_case_inner(cf: &CaseFile, baseline: Option<BaselineRow>, cfg: Config) -> Record {
    let mut rec = Record::new(cf);

    let text = match std::fs::read_to_string(&cf.path) {
        Ok(t) => t,
        Err(e) => {
            rec.status = Status::Failed;
            rec.note(format!("read failed: {e}"));
            return rec;
        }
    };

    // Parse through the module route; the timing covers source construction,
    // parsing, and typed narrowing.
    let t = Instant::now();
    let module = match parse_matpower(&text, &cf.path.to_string_lossy()) {
        Ok(module) => module,
        Err(e) => {
            rec.status = Status::Failed;
            rec.note(format!("parse failed: {e}"));
            return rec;
        }
    };
    rec.timings.parse_ms = ms(t);
    let net = module.value().clone();
    rec.buses = net.buses().len();
    rec.branches = net.branches().len();
    rec.gens = net.generators().len();

    // Declare each calculation through PowerIO.
    let t = Instant::now();
    let dc = DcOpfInstance::from_network(net.clone()).map_err(|e| e.to_string());
    rec.timings.build_dc_ms = ms(t);
    let t = Instant::now();
    let ac = AcOpfInstance::from_network(net.clone()).map_err(|e| e.to_string());
    let acpf = AcPfInstance::from_network(net).map_err(|e| e.to_string());
    rec.timings.build_ac_ms = ms(t);

    if dc.is_err() && ac.is_err() && acpf.is_err() {
        rec.status = Status::Failed;
        rec.note(format!(
            "problem declaration failed: dc={:?} ac={:?} acpf={:?}",
            dc.err(),
            ac.err(),
            acpf.err()
        ));
        return rec;
    }

    if let Ok(dc) = dc.as_ref() {
        run_dc(&mut rec, dc, &baseline);
    } else if let Some(e) = dc.as_ref().err() {
        rec.dc.error = Some(e.to_string());
        rec.raise(Status::Caveat);
    }

    if let Ok(ac) = ac.as_ref() {
        run_soc(&mut rec, ac, &baseline);
    } else if let Some(e) = ac.as_ref().err() {
        rec.soc.error = Some(e.to_string());
        rec.raise(Status::Caveat);
    }
    if let Ok(acpf) = acpf.as_ref() {
        run_acpf(&mut rec, acpf);
    } else if let Some(error) = acpf.as_ref().err() {
        rec.acpf.error = Some(error.to_string());
        rec.note(format!("AC power flow declaration failed: {error}"));
    }

    // Sensitivity parity, gated by size (the dense solve dominates at scale).
    if cfg.sample_sensitivity && cf.buses <= cfg.max_sens_bus {
        let t = Instant::now();
        if let Ok(dc) = dc.as_ref() {
            rec.parity.push(parity::dc_parity(dc));
        }
        if let Ok(acpf) = acpf.as_ref() {
            rec.parity.push(parity::ac_parity(acpf));
        }
        if let Ok(ac) = ac.as_ref() {
            rec.parity.push(parity::conic_parity(ac));
        }
        for p in &mut rec.parity {
            p.finalize();
        }
        rec.timings.sens_ms = ms(t);
        flag_parity(&mut rec);
    } else if cfg.sample_sensitivity {
        rec.note(format!(
            "sensitivity skipped: buses {} exceed --max-sens-bus {}",
            cf.buses, cfg.max_sens_bus
        ));
    }

    rec
}

fn run_dc(rec: &mut Record, dc: &DcOpfInstance, baseline: &Option<BaselineRow>) {
    let t = Instant::now();
    let out = match solve_instance(dc, &SolveRequest::default()) {
        Ok(o) => o,
        Err(e) => {
            rec.timings.dc_ms = ms(t);
            let msg = e.to_string();
            rec.dc.error = Some(msg.clone());
            // A primal-infeasible DC where the BASELINE also lists `inf.` is tellegen
            // correctly detecting infeasibility (the small-angle set), not a failure.
            let infeasible = msg.to_lowercase().contains("infeasible");
            let baseline_inf = baseline.as_ref().is_some_and(|b| b.dc.is_none());
            if infeasible && baseline_inf {
                rec.note(format!(
                    "DC infeasible ({msg}); consistent with BASELINE inf."
                ));
                rec.repro.dc = Repro::InfeasibleConsistent;
                rec.raise(Status::Caveat);
            } else {
                rec.note(format!("DC OPF solve failed: {msg}"));
                rec.raise(Status::Failed);
            }
            return;
        }
    };
    rec.timings.dc_ms = ms(t);

    // The public response includes each generator's constant cost term, so it
    // is directly comparable to the published BASELINE value.
    let objective = out.objective.unwrap_or(0.0);
    rec.dc.objective = Some(objective);
    rec.dc.iterations = Some(match &out.iterations {
        Some(Iterations::Ipm(trace)) => trace.len(),
        _ => 0,
    });

    // run_dc solves shed-off (to reproduce PGLib's infeasible verdict on unservable
    // cases), so a feasible DC OPF balances exactly (sum pg == sum demand) and never
    // sheds; the unservable case returns early above. `dc.shed_mw` stays None.

    if let Some(b) = baseline.as_ref().and_then(|b| b.dc) {
        rec.dc.baseline = Some(b);
        if b.abs() > 1e-9 {
            let rel = (objective - b).abs() / b.abs();
            rec.dc.rel_err = Some(rel);
            rec.repro.dc = if rel < DC_MATCH_TOL {
                Repro::Match
            } else {
                Repro::Mismatch
            };
        }
    }
}

/// Solve the SOCWR relaxation and record the gap/bound verdict.
fn run_soc(rec: &mut Record, instance: &AcOpfInstance, baseline: &Option<BaselineRow>) {
    let t = Instant::now();
    let request = SolveRequest {
        formulation: Problem::Socwr,
        ..Default::default()
    };
    let sol = match solve_ac_instance(instance, &request) {
        Ok(s) => s,
        Err(e) => {
            rec.timings.soc_ms = ms(t);
            rec.soc.error = Some(e.to_string());
            rec.note(format!("SOCWR solve failed: {e}"));
            rec.raise(Status::Caveat);
            return;
        }
    };
    rec.timings.soc_ms = ms(t);
    let objective = sol.objective.unwrap_or(0.0);
    rec.soc.objective = Some(objective);
    rec.soc.iterations = Some(match &sol.iterations {
        Some(Iterations::Ipm(trace)) => trace.len(),
        _ => 0,
    });

    if let Some(b) = baseline.as_ref() {
        rec.soc.baseline_qc_gap = b.qc_gap;
        // Bus-count cross-check against PGLib's published node count. The branch count
        // differs because the result lists only branches in the solved analysis view.
        let solved_buses = sol.w.as_deref().map_or(0, <[_]>::len);
        if b.nodes != 0 && b.nodes != solved_buses {
            rec.note(format!(
                "bus count {} differs from BASELINE nodes {}",
                solved_buses, b.nodes
            ));
        }
        if let Some(ac_ref) = b.ac {
            rec.soc.baseline_ac = Some(ac_ref);
            // Relaxation lower bound: socwr ≤ AC (up to solver tolerance).
            let ok = objective <= ac_ref * (1.0 + 1e-4) + 1e-6 * ac_ref.abs().max(1.0);
            rec.soc.bound_ok = Some(ok);
            if !ok {
                rec.note(format!(
                    "SOCWR bound violation: socwr {:.4e} > AC {:.4e}",
                    objective, ac_ref
                ));
                rec.raise(Status::Caveat);
            }
            if ac_ref.abs() > 1e-9 {
                let gap = (ac_ref - objective) / ac_ref * 100.0;
                rec.soc.gap_pct = Some(gap);
                if let Some(bg) = b.soc_gap {
                    rec.soc.baseline_soc_gap = Some(bg);
                    rec.soc.delta_gap = Some(gap - bg);
                }
            }
        }
    }

    // SOCWR reproduces the published relaxation when it is a valid lower bound whose gap
    // tracks the published SOC gap (same Jabr family).
    rec.repro.soc = match (rec.soc.bound_ok, rec.soc.delta_gap) {
        (Some(false), _) => Repro::Mismatch,
        (Some(true), Some(d)) if d.abs() < SOC_GAP_TOL => Repro::BoundMatch,
        (Some(true), _) => Repro::BoundLoose,
        (None, _) => Repro::Missing,
    };
}

fn run_acpf(rec: &mut Record, instance: &AcPfInstance) {
    let t = Instant::now();
    let request = SolveRequest {
        formulation: Problem::AcPf,
        ..Default::default()
    };
    match solve_ac_pf_instance(instance, &request) {
        Ok(sol) => {
            rec.timings.acpf_ms = ms(t);
            rec.acpf.converged = Some(true);
            if let Some(Iterations::Newton { count, residual }) = sol.iterations {
                rec.acpf.iterations = Some(count);
                rec.acpf.residual = Some(residual);
            }
        }
        Err(e) => {
            rec.timings.acpf_ms = ms(t);
            rec.acpf.converged = Some(false);
            rec.acpf.error = Some(e.to_string());
            // A PGLib OPF setpoint need not be a power flow point under an all PQ
            // flat start Newton solve; record it as a diagnostic, do not downgrade the
            // case (the OPF/sensitivity results stand on their own). `ac_pf` already names
            // the stage in its error, so report it verbatim rather than re-prefixing it.
            rec.note(e.to_string());
        }
    }
}

/// Raise a caveat only when adjoint and forward disagree beyond the solve consistency
/// bound — a genuine sign the differentiated system is off. Finite-difference outliers
/// are *expected* (a central difference straddling an active set kink, or the Jabr cone's
/// soft directions), are recorded per class in the parity table, and do not downgrade the
/// case: the analytic columns are validated by adjoint == forward, not by the FD.
fn flag_parity(rec: &mut Record) {
    for p in &rec.parity {
        if p.worst_adjoint_forward > 1e-3 {
            rec.notes.push(format!(
                "{}: adjoint−forward {:.2e} exceeds 1e-3",
                p.formulation, p.worst_adjoint_forward
            ));
            rec.status = match rec.status {
                Status::Solved => Status::Caveat,
                s => s,
            };
        }
    }
}
