//! Time one retained multiconductor fixed-point load update.
//!
//! Usage:
//!
//! ```text
//! cargo run --release -p tellegen --example mc_pf_session --features mc-pf -- case.json [scale]
//! ```

use std::{env, io::Write, path::PathBuf, process::ExitCode, time::Instant};

use serde::Serialize;
use tellegen::{parse_bmopf_instance, McLoadPowerEdit, McPfOptions, McPfSession};

#[derive(Serialize)]
struct SessionTiming {
    load_branches: usize,
    scale: f64,
    initial_iterations: usize,
    update_iterations: usize,
    factorization_count: usize,
    initial_ms: f64,
    update_ms: f64,
}

fn run(path: PathBuf, scale: f64) -> Result<(), String> {
    if !scale.is_finite() || scale < 0.0 {
        return Err("scale must be a finite non-negative number".to_owned());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let instance = parse_bmopf_instance(&text)?;

    let started = Instant::now();
    let mut session = McPfSession::new(instance, McPfOptions::default())?;
    let initial_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let initial_iterations = session.result().iterations;
    let branches = session.load_branches();
    let edits = branches
        .iter()
        .map(|branch| McLoadPowerEdit {
            load: branch.load.clone(),
            branch: branch.branch,
            p_w: branch.base_p_w * scale,
            q_var: branch.base_q_var * scale,
        })
        .collect::<Vec<_>>();

    let started = Instant::now();
    let update_iterations = session.replace_load_powers(&edits)?.iterations;
    let update_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let timing = SessionTiming {
        load_branches: branches.len(),
        scale,
        initial_iterations,
        update_iterations,
        factorization_count: session.factorization_count(),
        initial_ms,
        update_ms,
    };
    let output = serde_json::to_vec_pretty(&timing).map_err(|error| error.to_string())?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&output)
        .map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(path) = args.next() else {
        eprintln!("usage: mc_pf_session <BMOPF JSON path> [scale]");
        return ExitCode::from(2);
    };
    let scale = match args.next() {
        Some(value) => match value.to_string_lossy().parse::<f64>() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("mc_pf_session: invalid scale: {error}");
                return ExitCode::from(2);
            }
        },
        None => 1.001,
    };
    if args.next().is_some() {
        eprintln!("usage: mc_pf_session <BMOPF JSON path> [scale]");
        return ExitCode::from(2);
    }
    match run(PathBuf::from(path), scale) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mc_pf_session: {error}");
            ExitCode::from(1)
        }
    }
}
