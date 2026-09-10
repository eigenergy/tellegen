//! Small native multiconductor fixed-point runner.
//!
//! Usage:
//!
//! ```text
//! cargo run -p tellegen --example mc_pf --features mc-pf -- path/to/case.json [options.json]
//! ```
//!
//! The input must be BMOPF JSON and is parsed by PowerIO's pinned reader.  This executable intentionally
//! keeps the output as the typed Tellegen result so it is also useful for the
//! direct OpenDSS/BMOPFTools comparison harness.

use std::{env, io::Write, path::PathBuf, process::ExitCode};

use tellegen::mc_pf::parse_bmopf_instance;
use tellegen::{solve_mc_ac_pf_instance, McPfOptions};

fn run(path: PathBuf, options_path: Option<PathBuf>) -> Result<(), String> {
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let instance = parse_bmopf_instance(&text)?;
    let options = match options_path {
        Some(path) => serde_json::from_str::<McPfOptions>(
            &std::fs::read_to_string(path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("invalid options JSON: {error}"))?,
        None => McPfOptions::default(),
    };
    let result = solve_mc_ac_pf_instance(&instance, &options)?;
    let output = serde_json::to_vec_pretty(&result).map_err(|error| error.to_string())?;
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
        eprintln!("usage: mc_pf <BMOPF JSON path> [options JSON path]");
        return ExitCode::from(2);
    };
    let options = args.next().map(PathBuf::from);
    if args.next().is_some() {
        eprintln!("usage: mc_pf <BMOPF JSON path> [options JSON path]");
        return ExitCode::from(2);
    }
    match run(PathBuf::from(path), options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mc_pf: {error}");
            ExitCode::from(1)
        }
    }
}
