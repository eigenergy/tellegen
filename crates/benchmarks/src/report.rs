//! Result recording (`book/src/benchmark-results.md`): a machine-readable JSON + CSV artifact, one
//! row per `(case, variant)`, plus a committed markdown snapshot the mdBook Benchmarks
//! page renders without the corpus present. Records the toolchain provenance so the
//! deterministic solves are reproducible.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::record::{band, Record, Repro, Status};

/// Toolchain + run provenance.
#[derive(Clone, Debug, Serialize)]
pub struct Provenance {
    pub tellegen_commit: String,
    pub pglib_version: &'static str,
    pub pglib_archive: &'static str,
    pub rustc: String,
    pub clarabel: String,
    pub faer: String,
    pub powerio: String,
    pub os: &'static str,
    pub arch: &'static str,
    pub command_line: String,
    pub run_unix_time: u64,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Version of `name` from the workspace `Cargo.lock` (the line after its `[[package]]`
/// `name = "..."`).
fn locked_version(lock: &str, name: &str) -> String {
    let needle = format!("name = \"{name}\"");
    let mut hit = false;
    for line in lock.lines() {
        if hit {
            if let Some(v) = line.trim().strip_prefix("version = \"") {
                return v.trim_end_matches('"').to_string();
            }
        }
        hit = line.trim() == needle;
    }
    "unknown".into()
}

pub fn gather_provenance() -> Provenance {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let lock =
        std::fs::read_to_string(Path::new(manifest).join("../Cargo.lock")).unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Provenance {
        tellegen_commit: run("git", &["-C", manifest, "rev-parse", "--short", "HEAD"])
            .unwrap_or_else(|| "unknown".into()),
        pglib_version: "v23.07",
        pglib_archive: "arXiv:1908.02788",
        rustc: run("rustc", &["--version"]).unwrap_or_else(|| "unknown".into()),
        clarabel: locked_version(&lock, "clarabel"),
        faer: locked_version(&lock, "faer"),
        powerio: locked_version(&lock, "powerio"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        command_line: std::env::args().collect::<Vec<_>>().join(" "),
        run_unix_time: now,
    }
}

/// A flat per-`(case, variant)` row for the CSV artifact.
#[derive(Serialize)]
struct CsvRow<'a> {
    case: &'a str,
    variant: &'a str,
    buses: usize,
    branches: usize,
    gens: usize,
    status: &'a str,
    repro_dc: &'a str,
    repro_ac: &'a str,
    repro_soc: &'a str,
    dc_obj: Option<f64>,
    dc_baseline: Option<f64>,
    dc_rel_err: Option<f64>,
    dc_iters: Option<usize>,
    dc_shed_mw: Option<f64>,
    soc_obj: Option<f64>,
    soc_baseline_ac: Option<f64>,
    soc_gap_pct: Option<f64>,
    soc_baseline_gap: Option<f64>,
    soc_delta_gap: Option<f64>,
    soc_bound_ok: Option<bool>,
    soc_iters: Option<usize>,
    acopf_obj: Option<f64>,
    acopf_baseline_ac: Option<f64>,
    acopf_rel_err: Option<f64>,
    acopf_iters: Option<usize>,
    acopf_converged: Option<bool>,
    acpf_converged: Option<bool>,
    acpf_iters: Option<usize>,
    acpf_residual: Option<f64>,
    t_parse_ms: f64,
    t_build_dc_ms: f64,
    t_build_ac_ms: f64,
    t_dc_ms: f64,
    t_soc_ms: f64,
    t_acopf_ms: f64,
    t_acpf_ms: f64,
    t_sens_ms: f64,
    worst_adj_fwd: Option<f64>,
    worst_fd_clean: Option<f64>,
    worst_fd_coupled: Option<f64>,
    fd_columns: usize,
    notes: String,
}

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Solved => "solved",
        Status::Caveat => "caveat",
        Status::Failed => "failed",
        Status::Skipped => "skipped",
    }
}

/// Worst parity figures across the record's formulations (for the flat CSV row).
fn parity_rollup(r: &Record) -> (Option<f64>, Option<f64>, Option<f64>, usize) {
    if r.parity.is_empty() {
        return (None, None, None, 0);
    }
    let mut adj = 0.0f64;
    let mut clean = 0.0f64;
    let mut coupled = 0.0f64;
    let mut cols = 0;
    for p in &r.parity {
        adj = adj.max(p.worst_adjoint_forward);
        clean = clean.max(p.worst_fd_clean);
        coupled = coupled.max(p.worst_fd_coupled);
        cols += p.fd_columns;
    }
    (Some(adj), Some(clean), Some(coupled), cols)
}

pub fn write_csv(path: &Path, records: &[Record]) -> std::io::Result<()> {
    let mut w = csv::Writer::from_path(path)?;
    for r in records {
        let (adj, clean, coupled, cols) = parity_rollup(r);
        w.serialize(CsvRow {
            case: &r.case,
            variant: r.variant.tag(),
            buses: r.buses,
            branches: r.branches,
            gens: r.gens,
            status: status_str(r.status),
            repro_dc: r.repro.dc.mark(),
            repro_ac: r.repro.ac.mark(),
            repro_soc: r.repro.soc.mark(),
            dc_obj: r.dc.objective,
            dc_baseline: r.dc.baseline,
            dc_rel_err: r.dc.rel_err,
            dc_iters: r.dc.iterations,
            dc_shed_mw: r.dc.shed_mw,
            soc_obj: r.soc.objective,
            soc_baseline_ac: r.soc.baseline_ac,
            soc_gap_pct: r.soc.gap_pct,
            soc_baseline_gap: r.soc.baseline_soc_gap,
            soc_delta_gap: r.soc.delta_gap,
            soc_bound_ok: r.soc.bound_ok,
            soc_iters: r.soc.iterations,
            acopf_obj: r.acopf.objective,
            acopf_baseline_ac: r.acopf.baseline_ac,
            acopf_rel_err: r.acopf.rel_err,
            acopf_iters: r.acopf.iterations,
            acopf_converged: r.acopf.converged,
            acpf_converged: r.acpf.converged,
            acpf_iters: r.acpf.iterations,
            acpf_residual: r.acpf.residual,
            t_parse_ms: r.timings.parse_ms,
            t_build_dc_ms: r.timings.build_dc_ms,
            t_build_ac_ms: r.timings.build_ac_ms,
            t_dc_ms: r.timings.dc_ms,
            t_soc_ms: r.timings.soc_ms,
            t_acopf_ms: r.timings.acopf_ms,
            t_acpf_ms: r.timings.acpf_ms,
            t_sens_ms: r.timings.sens_ms,
            worst_adj_fwd: adj,
            worst_fd_clean: clean,
            worst_fd_coupled: coupled,
            fd_columns: cols,
            notes: r.notes.join("; "),
        })?;
    }
    w.flush()?;
    Ok(())
}

#[derive(Serialize)]
struct Artifact<'a> {
    provenance: &'a Provenance,
    records: &'a [Record],
}

pub fn write_json(path: &Path, prov: &Provenance, records: &[Record]) -> std::io::Result<()> {
    let art = Artifact {
        provenance: prov,
        records,
    };
    let s = serde_json::to_string_pretty(&art)?;
    std::fs::write(path, s)
}

// --- Markdown snapshot for the mdBook Benchmarks page -------------------------------

fn fnum(x: Option<f64>, sig: usize) -> String {
    match x {
        Some(v) => format!("{v:.*}", sig),
        None => "—".into(),
    }
}
fn esci(x: Option<f64>) -> String {
    match x {
        Some(v) => format!("{v:.4e}"),
        None => "—".into(),
    }
}

/// Render the full results artifact as the markdown the Benchmarks page embeds.
pub fn render_markdown(prov: &Provenance, records: &[Record]) -> String {
    let mut s = String::new();
    s.push_str("# Benchmark results\n\n");
    s.push_str(
        "Generated by the `benchmarks` harness over PGLib-OPF v23.07. Regenerate with \
         `cargo run -p benchmarks --release -- --out <dir>` and copy the snapshot here. \
         The solves are deterministic; the numbers reproduce on the recorded toolchain.\n\n",
    );

    // Provenance.
    s.push_str("## Provenance\n\n");
    s.push_str(&format!(
        "| tellegen | rustc | clarabel | faer | powerio | os/arch |\n\
         | --- | --- | --- | --- | --- | --- |\n\
         | `{}` | {} | {} | {} | {} | {}/{} |\n\n",
        prov.tellegen_commit,
        prov.rustc.replace("rustc ", ""),
        prov.clarabel,
        prov.faer,
        prov.powerio,
        prov.os,
        prov.arch,
    ));
    s.push_str(&format!(
        "PGLib {} ({}, CC BY 4.0). Command: `{}`.\n\n",
        prov.pglib_version, prov.pglib_archive, prov.command_line
    ));

    // Status summary.
    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    for r in records {
        *by_status.entry(status_str(r.status)).or_default() += 1;
    }
    s.push_str("## Coverage summary\n\n");
    s.push_str(&format!("{} (case, variant) rows.\n\n", records.len()));
    s.push_str("| status | count |\n| --- | --- |\n");
    for (k, v) in &by_status {
        s.push_str(&format!("| {k} | {v} |\n"));
    }
    s.push('\n');

    // Reproduction roll-up: the headline answer to "is tellegen reproducing PGLib?".
    s.push_str("## Reproduction of PGLib\n\n");
    s.push_str(&reproduction_summary(records));
    s.push('\n');

    // OPF correctness vs PGLib, per case.
    s.push_str("## OPF correctness vs PGLib BASELINE\n\n");
    s.push_str(
        "tellegen's objective against the published value, per formulation: the DC OPF ($/h, \
         constant cost included) vs the published DC, the exact nonlinear AC OPF vs the \
         published AC, and the SOCWR relaxation (a lower bound on AC). `Δgap` is tellegen's SOC \
         gap minus the published SOC gap; near zero is the steelman result (same Jabr family). \
         The **reproduces** column is `DC·AC·SOC`: ✓ matches the published objective (DC within \
         1%, AC within 0.1%), `inf✓` infeasible consistent with the published `inf.`, ✓lb a \
         valid lower bound whose gap matches the published SOC, lb a valid bound with a looser \
         gap, ✗ a mismatch (a converged but differing objective), ✗nc an AC OPF that did \
         not converge (no objective produced), — no baseline.\n\n",
    );
    s.push_str(
        "| case | var | buses | DC $/h | PGLib DC | AC OPF $/h | PGLib AC | SOCWR $/h | Δgap | reproduces |\n\
         | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: |\n",
    );
    for r in records.iter().filter(|r| r.status != Status::Skipped) {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {}·{}·{} |\n",
            r.case.replace("pglib_opf_", ""),
            r.variant.tag(),
            r.buses,
            fnum(r.dc.objective, 1),
            fnum(r.dc.baseline, 1),
            fnum(r.acopf.objective, 1),
            fnum(r.acopf.baseline_ac, 1),
            fnum(r.soc.objective, 1),
            fnum(r.soc.delta_gap, 2),
            r.repro.dc.mark(),
            r.repro.ac.mark(),
            r.repro.soc.mark(),
        ));
    }
    s.push('\n');

    // Sensitivity parity by size band.
    s.push_str("## Sensitivity parity (finite differences)\n\n");
    s.push_str(
        "Per size band. `adj−fwd` (worst) is the adjoint/forward solve-consistency bound — \
         the analytic guarantee, near machine precision. `FdClean` and `coupled` are the \
         central-difference relative errors per parity class (coupled = Jabr-cone soft \
         cells), shown as median/worst. The **median** is the validation signal: the typical \
         agreement in the smooth interior. The **worst** is an outlier where the two-sided \
         difference straddles a non-smooth point — an LMP at a congestion boundary, where the \
         sensitivity is genuinely one-sided and a central difference is not a valid check, or \
         a soft Jabr-cone direction. These are finite-difference limitations, not analytic \
         errors; the analytic columns are pinned by adjoint == forward and by the median. \
         Columns below the regularization floor are skipped.\n\n",
    );
    s.push_str(
        "| band | formulation | cases | cells | FD cols | worst adj−fwd | FdClean med/worst | coupled med/worst |\n\
         | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    s.push_str(&parity_band_table(records));
    s.push('\n');

    // Performance by size band.
    s.push_str("## Performance (wall time per stage)\n\n");
    s.push_str("Median milliseconds per stage, by size band. Solves are single-threaded.\n\n");
    s.push_str(
        "| band | cases | parse | build | DC | SOCWR | AC OPF | AC PF | sens |\n\
         | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    s.push_str(&perf_band_table(records));
    s.push('\n');

    // Coverage and limits.
    s.push_str("## Limits and skips\n\n");
    s.push_str(
        "Every capped, failed, or caveated case, with its reason (no silent truncation).\n\n",
    );
    s.push_str("| case | var | buses | status | notes |\n| --- | --- | ---: | --- | --- |\n");
    let mut any = false;
    for r in records
        .iter()
        .filter(|r| !matches!(r.status, Status::Solved))
    {
        any = true;
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r.case.replace("pglib_opf_", ""),
            r.variant.tag(),
            r.buses,
            status_str(r.status),
            r.notes.join("; "),
        ));
    }
    if !any {
        s.push_str("| — | — | — | — | every case solved cleanly |\n");
    }
    s.push('\n');
    s.push_str(&format!(
        "_{} rows; {} skipped/failed/caveat._\n",
        records.len(),
        records
            .iter()
            .filter(|r| !matches!(r.status, Status::Solved))
            .count()
    ));
    s
}

/// The corpus roll-up answering "is tellegen reproducing PGLib?" per formulation. A row
/// counts toward a formulation only when the BASELINE has a value to compare.
fn reproduction_summary(records: &[Record]) -> String {
    let count = |sel: &dyn Fn(&Record) -> Repro, pred: &dyn Fn(Repro) -> bool| {
        records.iter().filter(|r| pred(sel(r))).count()
    };
    let dc = |r: &Record| r.repro.dc;
    let ac = |r: &Record| r.repro.ac;
    let soc = |r: &Record| r.repro.soc;
    let has_base = |x: Repro| x != Repro::Missing;
    let reproduced = |x: Repro| matches!(x, Repro::Match | Repro::BoundMatch);
    let consistent = |x: Repro| matches!(x, Repro::InfeasibleConsistent | Repro::BoundLoose);
    let mismatch = |x: Repro| matches!(x, Repro::Mismatch | Repro::NonConvergence);

    let mut s = String::new();
    s.push_str(
        "Corpus roll-up of the per-case marks in the table below. \"Reproduced\" is an \
         objective match (DC within 1%, AC within 0.1%) or, for the SOCWR relaxation, a valid \
         lower bound whose gap matches the published SOC gap. \"Consistent\" is an acceptable \
         non-exact agreement: a DC the baseline also reports infeasible, or a SOCWR lower bound \
         with a looser gap. \"Mismatch\" is a differing objective, a non-converged AC OPF, or a \
         lower-bound violation.\n\n",
    );
    s.push_str("| formulation | reproduced | consistent | mismatch | with baseline |\n");
    s.push_str("| --- | ---: | ---: | ---: | ---: |\n");
    for (name, sel) in [
        ("DC OPF", &dc as &dyn Fn(&Record) -> Repro),
        ("AC OPF", &ac),
        ("SOCWR (lower bound)", &soc),
    ] {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            name,
            count(sel, &reproduced),
            count(sel, &consistent),
            count(sel, &mismatch),
            count(sel, &has_base),
        ));
    }
    s.push('\n');
    s
}

fn median(mut xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        0.5 * (xs[n / 2 - 1] + xs[n / 2])
    }
}

#[derive(Default)]
struct BandAgg {
    cases: usize,
    cells: usize,
    cols: usize,
    worst_adj: f64,
    worst_clean: f64,
    clean_medians: Vec<f64>,
    worst_coupled: f64,
    coupled_medians: Vec<f64>,
}

fn parity_band_table(records: &[Record]) -> String {
    let mut agg: BTreeMap<(&str, String), BandAgg> = BTreeMap::new();
    for r in records {
        for p in &r.parity {
            let e = agg
                .entry((band(r.buses), p.formulation.clone()))
                .or_default();
            e.cases += 1;
            e.cells += p.cells_supported;
            e.cols += p.fd_columns;
            e.worst_adj = e.worst_adj.max(p.worst_adjoint_forward);
            // A zero worst means the case had no column of that class — exclude its
            // (spurious zero) median from the band's typical figure.
            if p.worst_fd_clean > 0.0 {
                e.worst_clean = e.worst_clean.max(p.worst_fd_clean);
                e.clean_medians.push(p.median_fd_clean);
            }
            if p.worst_fd_coupled > 0.0 {
                e.worst_coupled = e.worst_coupled.max(p.worst_fd_coupled);
                e.coupled_medians.push(p.median_fd_coupled);
            }
        }
    }
    // "median / worst" cell, or "—" when the band has no column of that class.
    let cell = |medians: &[f64], worst: f64| -> String {
        if medians.is_empty() {
            "—".to_string()
        } else {
            format!(
                "{} / {}",
                esci(Some(median(medians.to_vec()))),
                esci(Some(worst))
            )
        }
    };
    let mut s = String::new();
    for ((b, f), a) in &agg {
        s.push_str(&format!(
            "| {b} | {f} | {} | {} | {} | {} | {} | {} |\n",
            a.cases,
            a.cells,
            a.cols,
            esci(Some(a.worst_adj)),
            cell(&a.clean_medians, a.worst_clean),
            cell(&a.coupled_medians, a.worst_coupled),
        ));
    }
    if agg.is_empty() {
        s.push_str("| — | — | — | — | — | — | — | — |\n");
    }
    s
}

fn perf_band_table(records: &[Record]) -> String {
    let bands = ["<100", "100–1k", "1k–10k", ">10k"];
    let mut s = String::new();
    for b in bands {
        let rs: Vec<&Record> = records
            .iter()
            .filter(|r| band(r.buses) == b && r.status != Status::Skipped)
            .collect();
        if rs.is_empty() {
            continue;
        }
        let col = |f: &dyn Fn(&Record) -> f64| median(rs.iter().map(|r| f(r)).collect());
        s.push_str(&format!(
            "| {b} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            rs.len(),
            col(&|r| r.timings.parse_ms),
            col(&|r| r.timings.build_dc_ms + r.timings.build_ac_ms),
            col(&|r| r.timings.dc_ms),
            col(&|r| r.timings.soc_ms),
            col(&|r| r.timings.acopf_ms),
            col(&|r| r.timings.acpf_ms),
            col(&|r| r.timings.sens_ms),
        ));
    }
    if s.is_empty() {
        s.push_str("| — | — | — | — | — | — | — | — | — |\n");
    }
    s
}
