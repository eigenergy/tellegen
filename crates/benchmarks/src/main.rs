//! PGLib-OPF validation and benchmark harness for tellegen.
//!
//! Drives tellegen's public API (`solve_json` / `solve_network` / `solve_prebuilt`,
//! `socwr_opf`, `ac_pf`, and the typed sensitivity engines) over the PGLib-OPF
//! v23.07 corpus, validates against two independent baselines — the published PGLib
//! reference solves (`BASELINE.md`) and finite-difference derivatives — and records
//! every result, skip, and cap. See the `book` (Benchmarks, Methodology) for the spec.
//!
//! Usage:
//! ```text
//! cargo run -p benchmarks --release -- [flags]
//!   --variants typ|api|sad|all   variant set (default: all)
//!   --max-bus N                  skip cases above N buses (default: unlimited)
//!   --max-sens-bus N             skip sensitivity sampling above N buses (default: 1500)
//!   --timeout SECS               per-case wall-clock guard (default: 180)
//!   --limit N                    run only the first N (smallest) cases
//!   --no-sens                    disable finite-difference parity sampling
//!   --pglib PATH                 corpus root (default: $PGLIB_OPF_PATH or ~/Datasets/pglib-opf)
//!   --out DIR                    artifact directory (default: target/pglib-bench)
//!   --book                       also copy the markdown snapshot into docs/src
//! ```

mod baseline;
mod corpus;
mod parity;
mod record;
mod report;
mod run;

use std::path::PathBuf;
use std::time::Duration;

use corpus::Variant;
use record::Status;
use run::Config;

struct Args {
    variants: Vec<Variant>,
    cfg: Config,
    limit: Option<usize>,
    pglib: Option<PathBuf>,
    out: PathBuf,
    book: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut variants = Variant::ALL.to_vec();
    let mut max_bus = 0usize;
    let mut max_sens_bus = 1500usize;
    let mut timeout = 180u64;
    let mut sample_sensitivity = true;
    let mut limit = None;
    let mut pglib = None;
    let mut out = PathBuf::from("target/pglib-bench");
    let mut book = false;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        // Accept `--flag value` and `--flag=value`.
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) => (f.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        let mut value = || inline.clone().or_else(|| it.next());
        match flag.as_str() {
            "--variants" => {
                let v = value().ok_or("--variants needs a value")?;
                variants = Variant::parse_set(&v).ok_or(format!("bad variant set '{v}'"))?;
            }
            "--max-bus" => {
                max_bus = value()
                    .ok_or("--max-bus needs a value")?
                    .parse()
                    .map_err(|_| "bad --max-bus")?;
            }
            "--max-sens-bus" => {
                max_sens_bus = value()
                    .ok_or("--max-sens-bus needs a value")?
                    .parse()
                    .map_err(|_| "bad --max-sens-bus")?;
            }
            "--timeout" => {
                timeout = value()
                    .ok_or("--timeout needs a value")?
                    .parse()
                    .map_err(|_| "bad --timeout")?;
            }
            "--limit" => {
                limit = Some(
                    value()
                        .ok_or("--limit needs a value")?
                        .parse()
                        .map_err(|_| "bad --limit")?,
                );
            }
            "--no-sens" => sample_sensitivity = false,
            "--pglib" => pglib = Some(PathBuf::from(value().ok_or("--pglib needs a value")?)),
            "--out" => out = PathBuf::from(value().ok_or("--out needs a value")?),
            "--book" => book = true,
            "-h" | "--help" => {
                println!("{}", include_str!("help.txt"));
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag '{other}' (try --help)")),
        }
    }

    Ok(Args {
        variants,
        cfg: Config {
            max_bus,
            max_sens_bus,
            timeout: Duration::from_secs(timeout),
            sample_sensitivity,
        },
        limit,
        pglib,
        out,
        book,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    let root = args.pglib.clone().unwrap_or_else(corpus::corpus_root);
    if !root.is_dir() {
        eprintln!(
            "PGLib corpus not found at {} (set $PGLIB_OPF_PATH). Skipping; nothing to benchmark.",
            root.display()
        );
        return;
    }

    let baseline_map = baseline::parse(&root.join("BASELINE.md"));
    if baseline_map.is_empty() {
        eprintln!(
            "warning: BASELINE.md not parsed at {}; correctness columns will be empty.",
            root.display()
        );
    }

    let mut cases = corpus::enumerate(&root, &args.variants);
    if let Some(n) = args.limit {
        cases.truncate(n);
    }
    if cases.is_empty() {
        eprintln!(
            "no .m files found under {} for variants {:?}",
            root.display(),
            args.variants.iter().map(|v| v.tag()).collect::<Vec<_>>()
        );
        return;
    }

    eprintln!(
        "Running {} (case, variant) pairs from {} (sensitivity {}; max-bus {}; max-sens-bus {}; timeout {}s)",
        cases.len(),
        root.display(),
        if args.cfg.sample_sensitivity { "on" } else { "off" },
        if args.cfg.max_bus == 0 { "unlimited".into() } else { args.cfg.max_bus.to_string() },
        args.cfg.max_sens_bus,
        args.cfg.timeout.as_secs(),
    );

    let total = cases.len();
    let mut records = Vec::with_capacity(total);
    for (i, cf) in cases.iter().enumerate() {
        let base = baseline_map.get(&(cf.case.clone(), cf.variant)).cloned();
        let rec = run::run_case(cf, base, args.cfg);
        eprintln!(
            "[{:>3}/{}] {:<28} {} {:>6}b  {:<7} dc={:>6.0} soc={:>6.0} acpf={:>6.0} sens={:>6.0}ms{}",
            i + 1,
            total,
            cf.case.replace("pglib_opf_", ""),
            cf.variant.tag(),
            rec.buses,
            status_label(rec.status),
            rec.timings.dc_ms,
            rec.timings.soc_ms,
            rec.timings.acpf_ms,
            rec.timings.sens_ms,
            first_note(&rec),
        );
        records.push(rec);
    }

    let prov = report::gather_provenance();

    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("error: cannot create {}: {e}", args.out.display());
        std::process::exit(1);
    }
    let json_path = args.out.join("results.json");
    let csv_path = args.out.join("results.csv");
    let md_path = args.out.join("results.md");
    let md = report::render_markdown(&prov, &records);
    let _ =
        report::write_json(&json_path, &prov, &records).map_err(|e| eprintln!("json write: {e}"));
    let _ = report::write_csv(&csv_path, &records).map_err(|e| eprintln!("csv write: {e}"));
    let _ = std::fs::write(&md_path, &md).map_err(|e| eprintln!("md write: {e}"));

    if args.book {
        let book_md =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/benchmark-results.md");
        let _ = std::fs::write(&book_md, &md).map_err(|e| eprintln!("book snapshot write: {e}"));
        eprintln!("wrote book snapshot to {}", book_md.display());
    }

    // Summary.
    let count = |s: Status| records.iter().filter(|r| r.status == s).count();
    eprintln!(
        "\nDone. {} solved, {} caveat, {} failed, {} skipped → {}",
        count(Status::Solved),
        count(Status::Caveat),
        count(Status::Failed),
        count(Status::Skipped),
        args.out.display(),
    );
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Solved => "ok",
        Status::Caveat => "caveat",
        Status::Failed => "FAILED",
        Status::Skipped => "skip",
    }
}

fn first_note(r: &record::Record) -> String {
    match r.notes.first() {
        Some(n) => format!("  ⤷ {n}"),
        None => String::new(),
    }
}
