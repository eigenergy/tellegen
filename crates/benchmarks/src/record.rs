//! The per-`(case, variant)` result record — one row of the validation matrix
//! (`docs/src/methodology.md`) — and its JSON/CSV-friendly sub-structs. The runner fills
//! these; the report module serializes them.

use serde::Serialize;

use crate::corpus::{CaseFile, Variant};

/// Outcome of a case. `Caveat` is solved-but-noteworthy (PF non-convergence, DC load
/// shedding, a relaxation bound violation, a parity outlier); the detail is in `notes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    Solved,
    Caveat,
    Failed,
    Skipped,
}

/// Wall time per stage, milliseconds. The harness times tellegen's public calls itself
/// (`std::time::Instant`); tellegen exposes iteration traces and residuals, not wall time.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Timings {
    pub parse_ms: f64,
    pub build_dc_ms: f64,
    pub build_ac_ms: f64,
    pub dc_ms: f64,
    pub soc_ms: f64,
    pub acopf_ms: f64,
    pub acpf_ms: f64,
    pub sens_ms: f64,
}

/// DC OPF result vs the BASELINE `DC ($/h)` column. `objective` includes the constant
/// cost term so it is comparable to the published value.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DcResult {
    pub objective: Option<f64>,
    pub iterations: Option<usize>,
    /// Total load shed (MW), reserved for a shedding-enabled DC sweep. Every `run_dc` path
    /// in the current harness solves with shedding disabled (PGLib parity reports the
    /// unservable case as infeasible rather than shedding), so no path sets `shed = true`:
    /// this is always `None` today and its CSV column is always empty. It exists so a future
    /// shed-on run can fill it, where a positive value would flag that the served case
    /// differs from the published one. Do not read it as evidence that no case ever shed.
    pub shed_mw: Option<f64>,
    pub baseline: Option<f64>,
    pub rel_err: Option<f64>,
    pub error: Option<String>,
}

/// Conic SOCWR result vs the BASELINE `AC ($/h)` / `SOC Gap (%)` columns.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SocResult {
    /// SOCWR objective, $/h incl. the constant cost term (a convex lower bound on AC).
    pub objective: Option<f64>,
    pub iterations: Option<usize>,
    /// Phasor's gap `(AC − socwr) / AC · 100`, using the baseline AC objective.
    pub gap_pct: Option<f64>,
    pub baseline_soc_gap: Option<f64>,
    /// The published QC relaxation gap, recorded for context (tellegen has no QC path).
    pub baseline_qc_gap: Option<f64>,
    /// `gap_pct − baseline_soc_gap`: near zero is the expected steelman result (same
    /// Jabr relaxation family).
    pub delta_gap: Option<f64>,
    pub baseline_ac: Option<f64>,
    /// `socwr ≤ AC · (1 + tol)`: the relaxation lower-bound property. `false` is a
    /// correctness failure.
    pub bound_ok: Option<bool>,
    pub error: Option<String>,
}

/// Full nonlinear AC OPF result vs the BASELINE `AC ($/h)` column — the exact optimum
/// PGLib publishes, which the SOCWR relaxation only lower-bounds. The objective already
/// includes the constant cost term.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AcOpfResult {
    pub objective: Option<f64>,
    pub iterations: Option<usize>,
    pub converged: Option<bool>,
    pub baseline_ac: Option<f64>,
    /// `|objective − baseline| / |baseline|`.
    pub rel_err: Option<f64>,
    pub error: Option<String>,
}

/// Whether tellegen reproduces the published PGLib value for one formulation — the answer
/// to "is tellegen reproducing PGLib?" at a glance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Repro {
    /// Objective matches the published value within tolerance.
    Match,
    /// SOCWR: a valid lower bound whose gap tracks the published SOC gap.
    BoundMatch,
    /// SOCWR: a valid lower bound, but the gap differs from the published one.
    BoundLoose,
    /// The published value is infeasible and tellegen agrees.
    InfeasibleConsistent,
    /// tellegen solved but the objective differs from the published value beyond tolerance.
    Mismatch,
    /// The solver did not converge, so tellegen produced no objective to compare. Distinct
    /// from `Mismatch`: a non-reproduction, but not a wrong optimum.
    NonConvergence,
    /// No baseline to compare against, or the stage was not run.
    #[default]
    Missing,
}

impl Repro {
    /// Compact mark for the markdown verdict column.
    pub fn mark(self) -> &'static str {
        match self {
            Repro::Match => "✓",
            Repro::BoundMatch => "✓lb",
            Repro::BoundLoose => "lb",
            Repro::InfeasibleConsistent => "inf✓",
            Repro::Mismatch => "✗",
            Repro::NonConvergence => "✗nc",
            Repro::Missing => "—",
        }
    }
}

/// Per-formulation reproduction verdict for one case.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ReproVerdict {
    pub dc: Repro,
    pub ac: Repro,
    pub soc: Repro,
}

/// AC power flow result: a Newton solve from the case setpoint. PGLib cases are OPF
/// points, so non-convergence is recorded as data, not counted as a tellegen failure.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AcPfResult {
    pub converged: Option<bool>,
    pub iterations: Option<usize>,
    pub residual: Option<f64>,
    pub error: Option<String>,
}

/// Finite-difference parity summary for one formulation on one case
/// (`docs/src/methodology.md`). Worst relative errors are reported per parity class; columns below the
/// regularization floor are skipped, not counted.
#[derive(Clone, Debug, Serialize)]
pub struct ParitySummary {
    pub formulation: String,
    /// `(operand, parameter)` cells probed and how many the formulation supports.
    pub cells_probed: usize,
    pub cells_supported: usize,
    /// Worst `|adjoint − forward|` over sampled cells (a solve-consistency bound).
    pub worst_adjoint_forward: f64,
    /// Columns finite-differenced (significant, above the floor).
    pub fd_columns: usize,
    /// Worst (outlier) relative FD error among `FdClean` cells (active power routed).
    pub worst_fd_clean: f64,
    /// Median (typical) relative FD error among `FdClean` cells.
    pub median_fd_clean: f64,
    /// Worst relative FD error among Jabr-coupled / soft cells.
    pub worst_fd_coupled: f64,
    /// Median relative FD error among Jabr-coupled / soft cells.
    pub median_fd_coupled: f64,
    pub notes: Vec<String>,
    /// Per-column relative errors, collected during the sweep and reduced to the worst /
    /// median scalars by [`finalize`](ParitySummary::finalize). Not serialized.
    #[serde(skip)]
    pub clean_errs: Vec<f64>,
    #[serde(skip)]
    pub coupled_errs: Vec<f64>,
}

impl ParitySummary {
    pub fn new(formulation: &str) -> Self {
        ParitySummary {
            formulation: formulation.to_string(),
            cells_probed: 0,
            cells_supported: 0,
            worst_adjoint_forward: 0.0,
            fd_columns: 0,
            worst_fd_clean: 0.0,
            median_fd_clean: 0.0,
            worst_fd_coupled: 0.0,
            median_fd_coupled: 0.0,
            notes: Vec::new(),
            clean_errs: Vec::new(),
            coupled_errs: Vec::new(),
        }
    }

    /// Reduce the collected per-column errors to worst (max) and median scalars.
    pub fn finalize(&mut self) {
        let reduce = |v: &mut Vec<f64>| -> (f64, f64) {
            if v.is_empty() {
                return (0.0, 0.0);
            }
            v.sort_by(f64::total_cmp);
            (*v.last().unwrap(), v[v.len() / 2])
        };
        (self.worst_fd_clean, self.median_fd_clean) = reduce(&mut self.clean_errs);
        (self.worst_fd_coupled, self.median_fd_coupled) = reduce(&mut self.coupled_errs);
    }
}

/// One row of the validation matrix.
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub case: String,
    pub variant: Variant,
    pub buses: usize,
    pub branches: usize,
    pub gens: usize,
    pub status: Status,
    pub dc: DcResult,
    pub soc: SocResult,
    pub acopf: AcOpfResult,
    pub acpf: AcPfResult,
    pub repro: ReproVerdict,
    pub parity: Vec<ParitySummary>,
    pub timings: Timings,
    pub notes: Vec<String>,
}

impl Record {
    pub fn new(cf: &CaseFile) -> Self {
        Record {
            case: cf.case.clone(),
            variant: cf.variant,
            buses: cf.buses,
            branches: 0,
            gens: 0,
            status: Status::Solved,
            dc: DcResult::default(),
            soc: SocResult::default(),
            acopf: AcOpfResult::default(),
            acpf: AcPfResult::default(),
            repro: ReproVerdict::default(),
            parity: Vec::new(),
            timings: Timings::default(),
            notes: Vec::new(),
        }
    }

    /// A case that was never solved: capped by `--max-bus` or timed out.
    pub fn skipped(cf: &CaseFile, reason: impl Into<String>) -> Self {
        let mut r = Record::new(cf);
        r.status = Status::Skipped;
        r.notes.push(reason.into());
        r
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        self.notes.push(msg.into());
    }

    /// Raise the status to at least `s` (Solved < Caveat < Failed; Skipped is set
    /// only at construction). Lets independent stages each downgrade the record.
    pub fn raise(&mut self, s: Status) {
        let rank = |s: Status| match s {
            Status::Solved => 0,
            Status::Caveat => 1,
            Status::Skipped => 2,
            Status::Failed => 3,
        };
        if rank(s) > rank(self.status) {
            self.status = s;
        }
    }
}

/// Size band for the scaling curve and per-band parity reporting.
pub fn band(buses: usize) -> &'static str {
    match buses {
        0..=99 => "<100",
        100..=999 => "100–1k",
        1000..=9999 => "1k–10k",
        _ => ">10k",
    }
}
