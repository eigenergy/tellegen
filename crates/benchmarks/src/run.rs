//! Per-case runner: drive tellegen's public API over one corpus file, time each stage,
//! compare against the BASELINE reference, and finite-difference the sensitivities.
//! A `--max-bus` filename cap (reproducible) and a per-case timeout (a safety net)
//! bound the giant cases; every skip is recorded with its reason.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use tellegen::{
    ac_pf, socwr_opf, AcNetwork, AcPolar, DcNetwork, Iterations, SocWrSolution, SolveRequest,
};

use crate::baseline::BaselineRow;
use crate::corpus::CaseFile;
use crate::parity;
use crate::record::{Record, Repro, Status};

/// A DC or AC objective within this relative tolerance of the published value reproduces
/// it. DC is looser (tellegen's DC carries a small flow regularization the PowerModels DC
/// baseline does not); the exact AC OPF should match the published AC optimum tightly.
const DC_MATCH_TOL: f64 = 1e-2;
const AC_MATCH_TOL: f64 = 1e-3;
/// The SOCWR reproduces the published SOC relaxation when its gap is within this many
/// percentage points of the published SOC gap (the same Jabr relaxation family).
const SOC_GAP_TOL: f64 = 0.5;
/// The AC OPF FD parity sweep re-solves the AC OPF many times per case (the heaviest work
/// in the harness), so it runs only on small cases; the differentiation is unit-tested.
const ACOPF_PARITY_MAX_BUS: usize = 150;

/// Which nonlinear AC OPF solver backend `run_acopf` drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcopfBackend {
    /// The default `interiors` MIPS interior point (BSD-3).
    Interiors,
    /// The `pounce` Ipopt port (filter line search + restoration; EPL-2.0).
    Pounce,
    /// Try `interiors` first, fall back to `pounce` on non-convergence. The two backends
    /// recover overlapping but not identical subsets of the hardest cases, so best-of-both
    /// reproduces more than either alone; the case counts as reproduced if either converges.
    Best,
}

/// Run-wide knobs (`docs/src/methodology.md`).
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Skip a case whose filename bus count exceeds this (0 = unlimited).
    pub max_bus: usize,
    /// Skip the nonlinear AC OPF stage above this bus count (the MIPS interior point is not
    /// built for the giants), while still running DC / SOCWR / PF. 0 = unlimited.
    pub max_acopf_bus: usize,
    /// Skip sensitivity sampling above this bus count (the dense solve is the memory
    /// bottleneck), while still solving the OPF/PF.
    pub max_sens_bus: usize,
    /// Per-case wall-clock guard.
    pub timeout: Duration,
    /// Whether to finite-difference the sensitivities at all.
    pub sample_sensitivity: bool,
    /// Which AC OPF backend solves the nonlinear stage.
    pub acopf_backend: AcopfBackend,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Run one case under a timeout. The work runs on a detached thread so a hung solve can be
/// abandoned, but the thread is not cancelled: a timed-out solve keeps running and holding
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

    // Parse.
    let t = Instant::now();
    let net = match powerio::parse_str(&text, "matpower") {
        Ok(p) => p.network,
        Err(e) => {
            rec.status = Status::Failed;
            rec.note(format!("parse failed: {e}"));
            return rec;
        }
    };
    rec.timings.parse_ms = ms(t);

    // Build the DC and AC models.
    let t = Instant::now();
    let dc = DcNetwork::from_network(&net);
    rec.timings.build_dc_ms = ms(t);
    let t = Instant::now();
    let ac = AcNetwork::from_network(&net);
    rec.timings.build_ac_ms = ms(t);

    if dc.is_err() && ac.is_err() {
        rec.status = Status::Failed;
        rec.note(format!(
            "model build failed: dc={:?} ac={:?}",
            dc.err(),
            ac.err()
        ));
        return rec;
    }

    // The constant cost term, shared across formulations; the DC objective omits it.
    let const_cost: f64 = ac.as_ref().map(|n| n.cc.iter().sum()).unwrap_or(0.0);

    if let Ok(dc) = dc.as_ref() {
        rec.buses = dc.n;
        rec.branches = dc.m;
        rec.gens = dc.k;
        run_dc(&mut rec, dc, const_cost, &baseline);
    } else if let Some(e) = dc.as_ref().err() {
        rec.dc.error = Some(e.to_string());
        rec.raise(Status::Caveat);
    }

    if let Ok(ac) = ac.as_ref() {
        rec.buses = ac.n;
        rec.branches = ac.m;
        rec.gens = ac.k;
        let soc = run_soc(&mut rec, ac, &baseline);
        if cfg.max_acopf_bus == 0 || cf.buses <= cfg.max_acopf_bus {
            run_acopf(&mut rec, ac, &baseline, cfg.acopf_backend, soc.as_ref());
        } else {
            rec.note(format!(
                "AC OPF skipped: buses {} exceed --max-acopf-bus {}",
                cf.buses, cfg.max_acopf_bus
            ));
        }
        run_acpf(&mut rec, ac);
    } else if let Some(e) = ac.as_ref().err() {
        rec.soc.error = Some(e.to_string());
        rec.raise(Status::Caveat);
    }

    // Sensitivity parity, gated by size (the dense solve dominates at scale).
    if cfg.sample_sensitivity && cf.buses <= cfg.max_sens_bus {
        let t = Instant::now();
        rec.parity.push(parity::dc_parity(&net));
        if let Ok(ac) = ac.as_ref() {
            rec.parity.push(parity::ac_parity(ac));
            rec.parity.push(parity::conic_parity(ac));
            if cf.buses <= ACOPF_PARITY_MAX_BUS {
                rec.parity.push(parity::acopf_parity(ac));
            }
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

fn run_dc(rec: &mut Record, dc: &DcNetwork, const_cost: f64, baseline: &Option<BaselineRow>) {
    let t = Instant::now();
    let out = match tellegen::solve_prebuilt(dc, &SolveRequest::default()) {
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

    let objective = out.objective.unwrap_or(0.0) + const_cost;
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

/// Solve the SOCWR relaxation, record the gap/bound verdict, and return the solution so
/// the AC OPF stage can warm-start from it (`None` if the relaxation did not solve).
fn run_soc(
    rec: &mut Record,
    ac: &AcNetwork,
    baseline: &Option<BaselineRow>,
) -> Option<SocWrSolution> {
    let t = Instant::now();
    let sol = match socwr_opf(ac) {
        Ok(s) => s,
        Err(e) => {
            rec.timings.soc_ms = ms(t);
            rec.soc.error = Some(e.to_string());
            rec.note(format!("SOCWR solve failed: {e}"));
            rec.raise(Status::Caveat);
            return None;
        }
    };
    rec.timings.soc_ms = ms(t);
    rec.soc.objective = Some(sol.objective);
    rec.soc.iterations = Some(sol.iterations.len());

    if let Some(b) = baseline.as_ref() {
        rec.soc.baseline_qc_gap = b.qc_gap;
        // Bus-count cross-check against PGLib's published node count. (The branch count
        // legitimately differs — tellegen counts in-service branches, the baseline counts
        // every edge in the file — so only the bus dimension is cross-checked here.)
        if b.nodes != 0 && b.nodes != ac.n {
            rec.note(format!(
                "bus count {} differs from BASELINE nodes {}",
                ac.n, b.nodes
            ));
        }
        if let Some(ac_ref) = b.ac {
            rec.soc.baseline_ac = Some(ac_ref);
            // Relaxation lower bound: socwr ≤ AC (up to solver tolerance).
            let ok = sol.objective <= ac_ref * (1.0 + 1e-4) + 1e-6 * ac_ref.abs().max(1.0);
            rec.soc.bound_ok = Some(ok);
            if !ok {
                rec.note(format!(
                    "SOCWR bound violation: socwr {:.4e} > AC {:.4e}",
                    sol.objective, ac_ref
                ));
                rec.raise(Status::Caveat);
            }
            if ac_ref.abs() > 1e-9 {
                let gap = (ac_ref - sol.objective) / ac_ref * 100.0;
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

    Some(sol)
}

/// Solve the full nonlinear AC OPF and compare its objective to the published `AC ($/h)`.
/// This is the exact optimum PGLib reports — the SOCWR only lower-bounds it — so a match
/// is tellegen reproducing the AC column. The objective already includes the constant cost.
fn run_acopf(
    rec: &mut Record,
    ac: &AcNetwork,
    baseline: &Option<BaselineRow>,
    backend: AcopfBackend,
    warm: Option<&SocWrSolution>,
) {
    let t = Instant::now();
    let baseline_ac = baseline.as_ref().and_then(|b| b.ac);
    // When the SOCWR relaxation solved, warm-start the nonlinear stage from it (the lever
    // for the near-infeasible giants the flat start cannot crack); otherwise fall back to
    // the flat-start entry point. Each backend's `_warm` variant tries the reconstructed
    // point first and the flat-start schedule after, so warm-starting never loses a case.
    let run_interiors =
        || warm.map_or_else(|| tellegen::acopf(ac), |w| tellegen::acopf_warm(ac, w));
    let run_pounce = || {
        warm.map_or_else(
            || tellegen::acopf_pounce(ac),
            |w| tellegen::acopf_pounce_warm(ac, w),
        )
    };
    let result = match backend {
        AcopfBackend::Interiors => run_interiors(),
        AcopfBackend::Pounce => run_pounce(),
        // Best-of-both: the permissive `interiors` first, `pounce` as the fallback. A case is
        // reproduced if either converges; only one that defeats both is a non-convergence.
        AcopfBackend::Best => run_interiors()
            .or_else(|e1| run_pounce().map_err(|e2| format!("interiors: {e1}; pounce: {e2}"))),
    };
    match result {
        Ok(sol) => {
            rec.timings.acopf_ms = ms(t);
            rec.acopf.objective = Some(sol.objective);
            rec.acopf.iterations = Some(sol.iterations.len());
            // `acopf` returns `Ok` only when the solve converged.
            rec.acopf.converged = Some(true);
            rec.acopf.baseline_ac = baseline_ac;
            if let Some(acref) = baseline_ac {
                if acref.abs() > 1e-9 {
                    let rel = (sol.objective - acref).abs() / acref.abs();
                    rec.acopf.rel_err = Some(rel);
                    if rel < AC_MATCH_TOL {
                        rec.repro.ac = Repro::Match;
                    } else {
                        rec.repro.ac = Repro::Mismatch;
                        rec.note(format!(
                            "AC OPF {:.1} differs from baseline AC {:.1} ({:.2}%)",
                            sol.objective,
                            acref,
                            rel * 100.0
                        ));
                        rec.raise(Status::Caveat);
                    }
                }
            }
        }
        Err(e) => {
            rec.timings.acopf_ms = ms(t);
            rec.acopf.converged = Some(false);
            rec.acopf.error = Some(e.to_string());
            rec.acopf.baseline_ac = baseline_ac;
            // `acopf` already names the stage in its error, so report it verbatim rather
            // than re-prefixing it ("AC OPF solve failed: AC OPF solve failed: ...").
            rec.note(e.to_string());
            // A finite published AC that tellegen could not produce is a non-reproduction,
            // but the solver not converging is distinct from computing a wrong optimum;
            // mark it as such so the roll-up and table do not read as a wrong objective.
            if baseline_ac.is_some() {
                rec.repro.ac = Repro::NonConvergence;
                rec.raise(Status::Caveat);
            }
        }
    }
}

fn run_acpf(rec: &mut Record, ac: &AcNetwork) {
    let t = Instant::now();
    match ac_pf(&AcPolar::new(), ac) {
        Ok(sol) => {
            rec.timings.acpf_ms = ms(t);
            rec.acpf.converged = Some(true);
            rec.acpf.iterations = Some(sol.iterations);
            rec.acpf.residual = Some(sol.residual);
        }
        Err(e) => {
            rec.timings.acpf_ms = ms(t);
            rec.acpf.converged = Some(false);
            rec.acpf.error = Some(e.to_string());
            // A PGLib OPF setpoint need not be a power flow point under an all-PQ
            // flat-start Newton solve; record it as a diagnostic, do not downgrade the
            // case (the OPF/sensitivity results stand on their own). `ac_pf` already names
            // the stage in its error, so report it verbatim rather than re-prefixing it.
            rec.note(e.to_string());
        }
    }
}

/// Raise a caveat only when adjoint and forward disagree beyond the solve-consistency
/// bound — a genuine sign the differentiated system is off. Finite-difference outliers
/// are *expected* (a central difference straddling an active-set kink, or the Jabr cone's
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
